# OpenCode TUI — How It's Written, Why It's Fast, Why It Feels Interactive

> Source-verified findings from `references/opencode` (anomalyco/opencode fork, `dev` branch,
> `@opencode-ai/tui` v1.18.15, `@opentui/core` v0.4.5).
> Companion docs: `DESIGN.md` (visual system), `rust-tui-implementation-guide.md` (Rust clone plan).
> See also the earlier `opencode-tui-architecture.md` for the file-by-file map.

---

## 1. How It's Written

### 1.1 The stack

| Layer | Technology | Where |
|---|---|---|
| Renderer | **OpenTUI** (`@opentui/core@0.4.5`) — a **native (Zig) terminal core** with TypeScript bindings. Cell buffers, diffing, layout, tree-sitter highlighting all run in native code. | `createCliRenderer(...)` |
| UI framework | **SolidJS** via `@opentui/solid` — JSX components (`<box>`, `<text>`, `<scrollbox>`) compiled to native renderables. **No virtual DOM** — fine-grained signals re-render only dependent nodes. | `packages/tui/src/app.tsx` |
| Keybindings | `@opentui/keymap` — modal, layer-filtered binding tables with leader-key sequences. | `packages/tui/src/keymap.tsx` |
| Orchestration | **Effect TS** — `Effect.fn("Tui.run")`, `Effect.acquireRelease` for renderer lifecycle, `Deferred` for shutdown signaling. | `packages/tui/src/app.tsx:186` |
| Syntax highlight | **tree-sitter** (native), driven by `CodeRenderable` / `MarkdownRenderable` with `streaming: true`. | `scrollback.surface.ts:154-184` |
| Server link | SSE (Server-Sent Events) via `@opencode-ai/sdk/v2` → `GlobalEvent` stream. | `packages/tui/src/context/sdk.tsx` |

### 1.2 Two entry points, one architecture

```
opencode            → rich TUI  (packages/tui/src/app.tsx)
                        full-screen alternate buffer, targetFps 60, mouse on,
                        talks to a server process over SSE

opencode run/--mini → direct mode (packages/opencode/src/cli/cmd/run/)
                        "split-footer" mode, targetFps 30, mouse off,
                        single session, in-process, serial prompt queue
```

The **direct mode is the one worth cloning first** — it is the fast, focused chat loop.

### 1.3 The split-footer layout (the core trick)

`runtime.lifecycle.ts:181-195`:

```ts
const renderer = await createCliRenderer({
  targetFps: 30,
  maxFps: 60,
  screenMode: "split-footer",          // terminal split in two zones
  footerHeight: 4,                     // bottom 4 rows = reactive zone
  externalOutputMode: "capture-stdout",
  exitOnCtrlC: false,                  // custom two-press Ctrl-C
})
```

```
┌────────────────────────────────────┐
│ SCROLLBACK — append-only           │  writeToScrollback() → rows are
│ (assistant text, diffs, tools)     │  committed once, NEVER re-rendered
├────────────────────────────────────┤
│ FOOTER — 4 rows, reactive          │  SolidJS store drives repaint:
│ (prompt / status / permissions)    │  the ONLY region that updates live
└────────────────────────────────────┘
```

### 1.4 Provider tree (rich mode)

~20 nested SolidJS contexts (`app.tsx:247-349`): Exit → Epilogue → ErrorBoundary →
Paths → TerminalEnv → Startup → Clipboard → Keymap → Args → KV → Toast → Route →
Config → PluginRuntime → **SDK** → Permission → Project → **Sync** → Data → **Theme**
→ Local → PromptStash → Dialog → Frecency → PromptHistory → PromptRef → Editor →
Location → App. Each context owns one concern; any component can reach any concern.

### 1.5 The central store

`context/sync.tsx` — one `createStore` holding sessions, messages, parts, todos,
permissions, questions, providers, config, VCS… The single source of truth. SSE
events mutate it; SolidJS re-renders only the subscribed nodes.

### 1.6 Commands & keymap

- Commands are named strings (`"session.list"`, `"model.cycle_recent"`, …) with
  `title`, `category`, `slashName`, `slashAliases`, `suggested`, `hidden`, `run()`.
- Components register bindings declaratively via `useBindings(() => ({ mode, bindings }))`.
- A **mode stack** (`keymap.tsx:53`) pushes/pops modes (`base`, `modal`, …); dialogs
  push `"modal"` so base bindings suspend automatically (`dialog.tsx:81-85`).
- Leader keys with timeout, key aliases (`enter`→`return`), escape-clears-pending,
  backspace-pops-pending — from `@opentui/keymap/addons/opentui`.

---

## 2. Why It's Fast

Nine concrete mechanisms, all verified in source:

### 2.1 Append-only scrollback
Once rows are committed (`surface.commitRows(start, end)`), they become terminal
scrollback — the renderer never touches them again. Thousands of lines of markdown
cost nothing per frame.

### 2.2 Retained streaming surfaces
`scrollback.surface.ts` — for a streaming assistant message, **one**
`MarkdownRenderable` is created and mutated with `active.content += chunk`.
Completed/stable blocks are committed to scrollback
(`renderable._stableBlockCount` → `commitMarkdownBlocks`); only the not-yet-stable
tail block repaints. Text entries commit `height - 1` rows so only the last line
stays mutable (`scrollback.surface.ts:240`).

### 2.3 Native core + incremental parsing
Cell diffing, layout and tree-sitter parsing run in Zig, not JS. Tree-sitter parses
incrementally — an appended token re-parses only the affected subtree.

### 2.4 Frame-rate caps matched to the medium
Direct mode targets **30fps** (a PTY is bandwidth-limited; 60fps would just queue
bytes). Rich mode targets 60. `targetFps`/`maxFps` are explicit renderer options.

### 2.5 Event coalescing in one-frame windows
`context/sdk.tsx` — SSE events arriving within **16ms** are queued and flushed in a
single SolidJS `batch()`, producing at most one re-render per frame regardless of
token rate.

### 2.6 Lazy per-session loading
Messages are fetched only when you navigate into a session
(`routes/session/index.tsx`), never all upfront.

### 2.7 Startup pipelining
The splash is written to scrollback **before** the footer module finishes loading:
`runtime.lifecycle.ts:214-228` — `const footerTask = import("./footer")` starts,
splash is queued, `await renderer.idle()`, then `await footerTask`. The first paint
contains branding before the heavy module graph resolves. Theme palette is prewarmed
(`app.tsx:241`: `void renderer.getPalette(...)`) so the `system` theme avoids a
first-paint fallback flash.

### 2.8 Fine-grained reactivity, no VDOM
SolidJS signals: a store mutation re-renders exactly the text nodes subscribed to
that slice. No tree diffing pass.

### 2.9 Serial turn loop
Direct mode runs one prompt at a time: send → stream → block on a `Deferred` until
the session reports `idle` → return to prompt (`runtime.queue.ts`). No concurrent
turn contention, no lock complexity.

---

## 3. Why It Feels Interactive

| Mechanism | Detail |
|---|---|
| **Modal keymap + mode stack** | Vim-like modes; dialogs push `"modal"`, prompt uses an insert layer; leader-key chords. |
| **Command palette** | Every command carries `slashName`/`slashAliases`; palette fuzzy-matches with **fuzzysort**; ranking boosted by **frecency** (frequency + recency, `prompt/frecency.tsx`). |
| **Dialog stack** | Absolute overlay (zIndex 3000) over a dimmed backdrop (`RGBA(0,0,0,150)`); Esc/Ctrl-C pops; opening remembers and restores the previously focused renderable (`dialog.tsx:87-103`). Backdrop click dismisses unless a text selection is active. |
| **Toasts** | Single-slot toast, top-right, variant-colored side borders, auto-dismiss (default 5s), `.unref()`'d timer so it never holds the process open. |
| **Two-press Ctrl-C** | First Ctrl-C clears the prompt draft; second within the window interrupts the running turn (`session.abort`); exit is a separate deliberate gesture. |
| **Copy-on-select mouse** | Mouse-up with an active selection copies to clipboard + toast; right-click copies too; selection suppresses dialog dismissal. |
| **Live footer views** | The 4-row footer swaps between prompt / permission / question / subagent-inspector / command-menu views without touching the scrollback. |
| **Terminal chrome** | Window title follows route + session title (`OC \| <title>`, 40-char ellipsis); dark/light mode tracked from the terminal's own palette and `ESC [ ? 997` notifications. |
| **External editor** | `$EDITOR` opens with renderer suspend/resume and SIGINT juggling so the terminal state survives (`runtime.lifecycle.ts:257-277`). |
| **Spinner with physics** | Knight-rider scanner: bidirectional sweep, hold frames at both ends, alpha-decay trail, bloom on the lead cell (`ui/spinner.ts`). |
| **Autocomplete** | `@` files, `/` commands, prompt history (up/down), and a prompt **stash** (park a draft, come back). |

---

## 4. How to Clone It (short version)

Full plan in `rust-tui-implementation-guide.md`. The essence:

1. **Renderer**: `ratatui` + `crossterm` (already rustchat deps). Ratatui's double
   buffer writes only changed cells — that replaces OpenTUI's native diffing.
2. **Layout**: replicate split-footer as two ratatui chunks — message list above,
   4-row reactive footer below. Completed messages live as pre-rendered
   `Vec<Line<'static>>`; the streaming message is one retained `String`.
3. **Event loop**: one `tokio::select!` over crossterm `EventStream`, a backend
   `mpsc` channel, and a ~33ms tick. Drain all pending backend events per tick and
   apply them in one state update — the direct analog of their 16ms `batch()`.
4. **State**: a single `AppState` mutated through a reducer (rustchat already has
   `state.rs` + `event.rs` with `AppEvent`/`BackendEvent` — the shape is right).
5. **Theme**: reuse OpenCode's theme JSON format verbatim (`defs` + `{dark, light}`
   variants + refs) with serde; detect dark/light with `terminal-colorsaurus`
   (already a rustchat dep).
