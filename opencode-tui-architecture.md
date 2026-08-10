# OpenCode CLI/TUI Architecture — Deep Dive

> Based on analysis of the OpenCode monorepo (`packages/tui`, `packages/opencode/src/cli/cmd/run/`)
> at revision `dev` branch, August 2026.

## Overview

OpenCode has **two distinct TUI entry points** that share an architecture but differ in
integration depth:

| Entry | Location | What it is |
|---|---|---|
| **Rich TUI** | `packages/tui/src/app.tsx` | Full SolidJS+OpenTUI SPA — routes, dialogs, session browser, sidebar. Talks to a server process via SDK. |
| **Direct/Mini mode** | `packages/opencode/src/cli/cmd/run/` | Single-session split-footer renderer. Used by `opencode --mini`. No separate server — all in-process. |

Both use the same rendering framework (**OpenTUI**, a custom SolidJS-based terminal UI
framework), the same SDK client (`@opencode-ai/sdk/v2`), and the same keybinding system. The
direct mode is the one most relevant to cloning — it's the fast, focused codewhale session.

---

## 1. The Rendering Engine: OpenTUI

OpenCode does **not** use Ink/React. It uses **OpenTUI**, a proprietary SolidJS-based
terminal renderer. Key components:

### 1.1 CliRenderer (the core)

```ts
// packages/opencode/src/cli/cmd/run/runtime.lifecycle.ts:181-195
const renderer = await createCliRenderer({
  stdin: source.stdin,
  targetFps: 30,      // <-- moderate FPS, not 60
  maxFps: 60,
  useMouse: false,
  autoFocus: false,
  openConsoleOnError: false,
  exitOnCtrlC: false,
  screenMode: "split-footer",       // <-- KEY: split layout
  footerHeight: FOOTER_HEIGHT,      // 4 rows for footer
  externalOutputMode: "capture-stdout",
  consoleMode: "disabled",
  clearOnShutdown: false,
})
```

**Critical design decisions in the renderer:**

- **`targetFps: 30`** — Doesn't try to run at 60fps. Terminal rendering is bandwidth-limited
  by the PTY; 30fps is enough for smooth streaming text.
- **`screenMode: "split-footer"`** — The terminal is split into two regions: a scrollback
  area (everything above the bottom 4 rows) and a fixed footer. Only the footer can
  **repaint**; the scrollback is append-only.
- **`externalOutputMode: "capture-stdout"`** — External process output is captured and
  routed through the scrollback, not sprayed onto the terminal.
- **`consoleMode: "disabled"`** — The debug console overlay is off by default in direct
  mode.

### 1.2 SolidJS rendering bridge

```ts
// @opentui/solid provides:
//   render(component, renderer) — mounts a SolidJS tree into the terminal
//   useRenderer() — access CliRenderer from any component
//   useTerminalDimensions() — reactive width/height signals

// JSX primitives map to OpenTUI renderables:
//   <box>   → BoxRenderable
//   <text>  → TextRenderable
//   <scrollbox> → ScrollBoxRenderable (the main scrollback surface)
```

The rich TUI mounts one Solid tree for the entire app. The direct mode mounts a **SolidJS
tree just for the footer** — the scrollback is managed imperatively, not reactively.

---

## 2. The Split-Footer Architecture (The Secret Sauce)

This is the most important design pattern. It's what makes OpenCode feel instant even
during heavy streaming.

### 2.1 Two zones, two update models

```
┌──────────────────────────────────┐
│                                  │
│    SCROLLBACK (append-only)      │  ← Imperative: writeToScrollback(),
│                                  │     commitRows(). Never re-renders.
│                                  │     Managed by RunScrollbackStream.
│                                  │
│                                  │
│                                  │
├──────────────────────────────────┤
│  FOOTER (4 rows, reactive)       │  ← SolidJS signals + store.
│  prompt / status / permissions   │     Only this region repaints.
└──────────────────────────────────┘
```

**Why this matters:**

- The scrollback can grow to thousands of lines of markdown, code blocks, and diff output.
  If you re-render it on every keystroke or stream chunk, you'd choke.
- By making it **append-only**, new content is just `writeToScrollback(entry)` followed by
  `commitRows(start, end)` — the renderer only repaints the new rows.
- The footer (prompt, status bar, permission dialogs) is the only surface that needs
  reactivity, and it's capped at 4 rows.

### 2.2 Scrollback streaming

```ts
// packages/opencode/src/cli/cmd/run/scrollback.surface.ts

class RunScrollbackStream {
  private active: ActiveEntry | undefined  // currently streaming entry (e.g. assistant text)

  append(commits: StreamCommit[]) {
    for (const commit of commits) {
      const body = entryBody(commit)
      if (entryCanStream(body)) {
        // For streaming entries (assistant text, reasoning, tool progress):
        // Create a retained Surface that we can update in-place
        if (!this.active || !sameEntryGroup(this.active.commit, commit)) {
          this.flushActive()
          this.active = this.createActive(commit, body)
        }
        // Append text to the active surface without full re-layout
        this.active.content += commit.text
        this.active.renderable.setText(this.active.content)
        this.commitActiveBlocks()
      } else {
        // For static entries: write once, never touch again
        this.flushActive()
        this.writeStatic(commit, body)
      }
    }
  }
}
```

**The streaming trick:** For assistant text that's arriving token-by-token, OpenTUI
creates a single `MarkdownRenderable` or `TextRenderable`, then calls
`renderable.setText(newContent)` as each chunk arrives. The renderable manages its own
incremental layout — it only re-lays-out the changed blocks, not the entire scrollback.

### 2.3 How stream commits flow

```
SDK GlobalEvent stream
  │
  ▼
stream.transport.ts  ← Effect-based event loop, one-at-a-time prompt turns
  │
  │  reduceSessionData() → SessionData (permissions, questions, todos, tools)
  │  reduceSubagentData() → SubagentData (parallel sub-agent sessions)
  │
  ▼
syncFooter() → writeSessionOutput()
  │
  ├── scrollback.append(commits)   ← RunScrollbackStream
  │     └── renderer.writeToScrollback() or surface.commitRows()
  │
  └── footer.event({ patch, view, subagent })
        └── SolidJS store update (triggers reactive footer re-render)
```

---

## 3. The Event/State Sync System

### 3.1 SDK SSE connection (rich TUI)

```ts
// packages/tui/src/context/sdk.tsx
// Long-lived SSE connection, batched event delivery with 16ms coalescing

const handleEvent = (event: GlobalEvent) => {
  queue.push(event)
  const elapsed = Date.now() - last
  if (timer) return
  // Batch events that arrive within 16ms into one flush
  if (elapsed < 16) {
    timer = setTimeout(flush, 16)
    return
  }
  flush()
}
```

Events from the server (new messages, part deltas, session status changes, permission
requests) stream in via SSE. They're **coalesced in 16ms batches** (one frame budget) and
then dispatched through a SolidJS `batch()` call so all store mutations result in a single
re-render.

### 3.2 Sync context (the central store)

```ts
// packages/tui/src/context/sync.tsx
// One giant reactive store holding everything the TUI knows

const [store, setStore] = createStore<{
  status: "loading" | "partial" | "complete"
  provider: Provider[]
  agent: Agent[]
  session: Session[]
  message: { [sessionID: string]: Message[] }      // sorted by time
  part: { [messageID: string]: Part[] }             // text, tool, reasoning parts
  todo: { [sessionID: string]: Todo[] }
  permission: { [sessionID: string]: PermissionRequest[] }
  question: { [sessionID: string]: QuestionRequest[] }
  session_status: { [sessionID: string]: SessionStatus }
  session_diff: { [sessionID: string]: SnapshotFileDiff[] }
  lsp: LspStatus[]
  mcp: { [key: string]: McpStatus }
  config: Config
  vcs: VcsInfo
}>({ /* defaults */ })
```

This is the single source of truth. Every component reads from it via reactive accessors
(SolidJS `createMemo`). When events come in, the event handler updates the store, and
SolidJS's fine-grained reactivity re-renders only the affected DOM/text nodes.

### 3.3 Session-specific sync (rich TUI)

When navigating to a session route:

```ts
// packages/tui/src/routes/session/index.tsx:287-325
createEffect(() => {
  const sessionID = route.sessionID
  // 1. Fetch session metadata
  const result = await sdk.client.session.get({ sessionID })
  // 2. Switch workspace if needed
  project.workspace.set(result.data.workspaceID)
  // 3. Lazy-sync messages for this session on demand
  await sync.session.sync(sessionID)
  // 4. Scroll to bottom
  scroll.scrollBy(100_000)
})
```

Messages are **lazy-loaded per session** — the sync context fetches them on-demand when
you navigate to a session, rather than loading everything upfront.

---

## 4. The Prompt Queue (Direct Mode)

```ts
// packages/opencode/src/cli/cmd/run/runtime.queue.ts

export async function runPromptQueue(
  queue: AsyncIterable<RunPrompt | "close">,
  transport: SessionTransport,
  footer: FooterApi,
) {
  for await (const next of queue) {
    if (next === "close") break

    // 1. Show "thinking" state in footer
    await footer.event({
      patch: { status: "thinking...", phase: "running" }
    })

    // 2. Run the prompt turn — blocks until session becomes idle
    await transport.runPromptTurn({
      agent, model, variant,
      prompt: next,
      files, includeFiles,
      onVisibleOutput: (anchor) => { /* scrolling logic */ },
      signal: abortController.signal,
    })

    // 3. Back to prompt view after completion
    await footer.event({ view: { type: "prompt" } })
  }
}
```

In direct mode, turns are **serial** — one prompt at a time. The `runPromptTurn` sends the
prompt via SDK, then blocks on a Deferred that resolves when the session becomes idle
(`session.status` event with `status.type === "idle"`). While waiting, stream events
continue to arrive and the scrollback updates live.

### Interrupt handling

```ts
// Two-press pattern: first Ctrl-c clears the draft, second interrupts
footer.onInterrupt = () => {
  if (promptHasText) { footer.clearPrompt(); return }
  if (alreadyInterruptedRecently) { footer.requestExit(); return }
  sdk.session.abort({ sessionID })
}
```

---

## 5. The Component/Context Tree (Rich TUI)

```
render()
  └── ExitProvider
       └── EpilogueProvider
            └── ErrorBoundary
                 └── TuiPathsProvider
                      └── TuiTerminalEnvironmentProvider
                           └── TuiStartupProvider
                                └── ClipboardProvider
                                     └── OpencodeKeymapProvider
                                          └── ArgsProvider
                                               └── KVProvider
                                                    └── ToastProvider
                                                         └── RouteProvider
                                                              └── TuiConfigProvider
                                                                   └── PluginRuntimeProvider
                                                                        └── SDKProvider
                                                                             └── PermissionProvider
                                                                                  └── ProjectProvider
                                                                                       └── SyncProvider
                                                                                            └── DataProvider
                                                                                                 └── ThemeProvider
                                                                                                      └── LocalProvider
                                                                                                           └── ... → App
```

**This is a deeply nested context tree** — 20+ providers. Each provider is a SolidJS
context that makes a specific concern available to any descendant. This is React's context
pattern ported to SolidJS.

Key providers:

- **SDKProvider** — Creates the OpenCode SDK client, manages the SSE connection
- **SyncProvider** — The central reactive data store (sessions, messages, parts, providers)
- **RouteProvider** — Navigation state (home, session/:id, plugin/:id)
- **ThemeProvider** — Terminal palette detection, color scheme
- **LocalProvider** — Per-user preferences (selected agent, model, variant)
- **KVProvider** — Persistent key-value store backed by the server
- **ToastProvider** — Transient notification toasts
- **DialogProvider** — Modal dialog stack

---

## 6. The Keymap/Command System

```ts
// packages/tui/src/keymap.tsx

// Based on @opentui/keymap — a modal keybinding system
// Conceptually similar to Vim modes or tmux prefix

const OPENCODE_BASE_MODE = "base"

// Modes: base, insert (in textarea), command-palette, etc.
// Layer fields filter bindings by mode: { mode: "base" } or { mode: "insert" }

// Commands are named string identifiers:
//   "session.list", "session.new", "model.list", "agent.cycle",
//   "command.palette.show", "prompt.submit", "prompt.editor", etc.

// Bindings are defined in config files and compiled at startup.
// The command palette shows all visible commands with their keybindings.
```

Commands are the **action interface**. Components register keybindings via `useBindings()`
which returns command→handler pairs. The keymap system intercepts key events, matches them
against binding tables (filtered by the current mode), and dispatches to the matching
command handler.

---

## 7. What Makes It Fast — Summary

| Factor | How OpenCode does it |
|---|---|
| **Append-only scrollback** | Split-footer mode. Scrollback never re-renders; only the 4-row footer can repaint. |
| **Incremental text update** | Streaming entries use `renderable.setText()` which does block-level incremental layout. |
| **30fps target** | Terminal PTY bandwidth is the bottleneck. 60fps would waste cycles. |
| **16ms event batching** | SSE events are coalesced into 16ms windows, then flushed in one `batch()`. |
| **Lazy message loading** | Messages are fetched per-session on navigation, not all upfront. |
| **SolidJS fine-grained reactivity** | Store mutations only re-render the specific DOM nodes that depend on that slice of state. No virtual DOM diffing. |
| **Serial prompt turns** | Direct mode processes one prompt at a time with Deferred-based synchronization. No concurrent turn contention. |
| **External output capture** | `capture-stdout` mode routes subprocess output through the scrollback pipeline instead of spraying it raw. |
| **Retained streaming surfaces** | Rather than creating new renderables for each token chunk, a single retained `MarkdownRenderable` is updated in-place. |

---

## 8. The Key Files Map

```
packages/tui/src/
  app.tsx                          ← Rich TUI entry point, provider tree, route switch
  keymap.tsx                       ← Modal keybinding system
  context/
    sync.tsx                       ← Central reactive data store
    sdk.tsx                        ← SDK client + SSE event stream
    route.tsx                      ← Navigation (home | session/:id)
    theme.tsx                      ← Terminal palette detection
    local.tsx                      ← User preferences (agent, model)
    project.tsx                    ← Current project/workspace
    kv.tsx                         ← Persistent settings
    clipboard.tsx                  ← System clipboard integration
  routes/
    session/index.tsx              ← Session view (2725 lines — the big one)
    session/footer.tsx             ← Bottom status bar
    session/sidebar.tsx            ← Session sidebar with file tree
    session/permission.tsx         ← Permission request UI
    session/question.tsx           ← Question request UI
    session/subagent-footer.tsx    ← Sub-agent tab bar
  component/
    prompt/index.tsx               ← Input prompt with autocomplete (1716 lines)
    prompt/autocomplete.tsx        ← File/command autocomplete
    prompt/history.tsx             ← Prompt history (up/down)
    dialog-*.tsx                   ← All modal dialogs
  ui/
    dialog.tsx                     ← Dialog stack manager
    toast.tsx                      ← Toast notifications

packages/opencode/src/cli/cmd/run/
  runtime.ts                       ← Top-level orchestrator for direct mode
  runtime.lifecycle.ts             ← Renderer + footer creation, SIGINT handling
  runtime.boot.ts                  ← Boot sequence (config, model, session)
  runtime.queue.ts                 ← Serial prompt queue
  runtime.stdin.ts                 ← Interactive stdin setup
  stream.transport.ts              ← Effect-based event loop + prompt turns
  stream.ts                        ← Bridge between reducers and footer
  scrollback.surface.ts            ← Retained streaming scrollback
  scrollback.writer.tsx            ← SolidJS scrollback entry rendering
  scrollback.shared.ts             ← Color/syntax helpers
  footer.ts                        ← RunFooter class (1129 lines)
  footer.view.tsx                  ← Footer view routing (prompt/status/permission/question)
  footer.prompt.tsx                ← Direct-mode prompt UI
  footer.command.tsx               ← Command palette in footer
  footer.permission.tsx            ← Permission prompt in footer
  footer.question.tsx              ← Question prompt in footer
  footer.subagent.tsx              ← Sub-agent inspector in footer
  session-data.ts                  ← Session data reducer
  subagent-data.ts                 ← Sub-agent data reducer
  theme.ts                         ← RunTheme resolution
  types.ts                         ← All shared types
```
