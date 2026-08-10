# Cloning the OpenCode TUI in Rust — Implementation Guide

> Target: this repo (`rustchat` fork). Ratatui 0.29 + crossterm 0.28 + tokio are
> already in `Cargo.toml`; `src/tui/` already has `app.rs`, `event.rs`, `state.rs`,
> `theme.rs`, `kv.rs`, `routes/`, `widgets/`, `backend/`.
> This guide maps each OpenCode mechanism to a simple Rust equivalent.
> Read `opencode-tui-how-it-works.md` first for the "what/why"; this is the "how".

---

## 1. Concept Mapping — OpenCode → Rust

| OpenCode (TS/SolidJS) | Rust equivalent | Notes |
|---|---|---|
| OpenTUI `CliRenderer` (Zig native) | **ratatui** `Terminal<CrosstermBackend>` | ratatui's double buffer only writes changed cells → native-diffing equivalent |
| `screenMode: "split-footer"`, footer 4 rows | `Layout::vertical([Min(0), Length(4)])` | transcript above, reactive footer below |
| Append-only scrollback + `commitRows` | Completed messages stored as **pre-rendered `Vec<Line<'static>>`**; render only the visible window | See §3 |
| Retained `MarkdownRenderable` (streaming) | One retained `String` for the active message, re-highlighted per frame | See §4 |
| SolidJS store + fine-grained signals | Single `AppState` struct + reducer over events | rustchat's `state.rs` + `event.rs` already match this shape |
| 16ms SSE `batch()` coalescing | `mpsc::try_recv()` drain loop inside a 33ms tick | See §5 |
| `@opentui/keymap` modes + mode stack | `enum Mode { Base, Insert, Modal }` + `Vec<Mode>` stack | See §6 |
| Command registry (`slashName`, `run()`) | `struct Command { name, title, slash, aliases, run: fn(&mut AppState) }` | palette = fuzzy filter over `&[Command]` |
| Theme JSON (`defs`, `{dark,light}` refs) | serde structs + recursive resolver | You can load OpenCode's own `.json` themes. See §7 |
| `terminalMode()` luminance check | **terminal-colorsaurus** (already a dep) | `theme_mode()` → Dark/Light |
| Dialog stack (zIndex 3000) | Render overlays **after** base layout, last-on-top `Vec<Dialog>` | See §8 |
| Toast single-slot | `Option<Toast>` + `Instant` deadline | trivial |
| Knight-rider spinner | 8-cell string, frame counter → position + per-cell alpha→color lerp | See §9 |
| tree-sitter syntax | **syntect** (already a dep) — highlight once at commit time | streaming code: re-highlight only the tail block |
| Effect TS lifecycle (`acquireRelease`) | `scopeguard`/Drop guard restoring terminal | panic hook must restore too |
| SSE stream (`reqwest-eventsource`, already a dep) | `tokio::sync::mpsc<BackendEvent>` from a reader task | rustchat `backend/` already has `subscribe()` |

**Deliberately dropped** (keep it simple): plugin host/slots, workspaces, sub-agent
inspector, mouse copy-on-select, Kitty keyboard protocol, external `$EDITOR`
suspend/resume. Add later; none affect the core feel.

---

## 2. The Event Loop (the heart)

One task, one loop. Everything is an `AppEvent` on one channel — this is what makes
the UI deterministic and testable.

```rust
// src/tui/app.rs — shape only
pub async fn run(config: GlobalConfig) -> Result<()> {
    let _guard = TerminalGuard::enter()?;          // raw mode + alt screen, Drop restores
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut state = AppState::new(Theme::detect(), TuiPreferences::load());
    let mut backend = create_backend(config)?;      // your existing backend/
    let mut backend_rx = backend.subscribe();

    let (input_tx, mut input_rx) = mpsc::unbounded_channel();
    spawn_input_reader(input_tx);                   // blocking event::read() → tx (you have this)

    let mut ticker = tokio::time::interval(Duration::from_millis(33)); // 30 fps cap

    loop {
        tokio::select! {
            Some(ev) = input_rx.recv() => state.reduce(AppEvent::from(ev)),
            Some(ev) = backend_rx.recv() => {
                // COALESCING: this event + everything already queued → one update
                state.reduce(AppEvent::Backend(ev));
                while let Ok(next) = backend_rx.try_recv() {
                    state.reduce(AppEvent::Backend(next));
                }
            }
            _ = ticker.tick() => { /* spinner frame, toast expiry */ }
        }

        if state.dirty {
            terminal.draw(|f| ui(f, &mut state))?;
            state.dirty = false;
        }
        if state.should_quit { break }
    }
    Ok(())
}
```

Three rules that reproduce OpenCode's feel:

1. **`dirty` flag** — draw only when state changed. No redraw storm on idle ticks.
2. **`try_recv` drain** — the analog of their 16ms `batch()`: 100 token deltas in one
   tick produce one state update and one draw, not 100.
3. **30–33ms tick** — matches `targetFps: 30`. The terminal is the bottleneck; a
   faster loop only queues bytes.

Input reader (you already have `spawn_input_reader`): a plain thread doing
`crossterm::event::read()` and forwarding into the unbounded channel. (Alternative:
enable crossterm's `event-stream` feature and use `EventStream` directly in the
`select!`. Both fine; the thread needs no extra feature.)

---

## 3. Split-Footer Layout + Append-Only Transcript

OpenCode's split-footer exists because re-rendering a huge scrollback per frame is
slow. In ratatui the equivalent trick is **data structure**, not terminal mode:

```rust
struct Transcript {
    /// Completed entries — rendered ONCE, then only re-wrapped on resize.
    committed: Vec<Vec<Line<'static>>>,
    /// The one live entry (streaming assistant text / running tool).
    active: Option<ActiveEntry>,
    scroll: ScrollState,
}
```

Render:

```rust
fn ui(f: &mut Frame, state: &mut AppState) {
    let [body, footer] = Layout::vertical([Constraint::Min(0), Constraint::Length(4)])
        .areas(f.area());

    render_transcript(f, body, state);   // Paragraph::new(visible_lines).scroll(...)
    render_footer(f, footer, state);     // prompt / status / permission / palette

    for dialog in &state.dialogs { render_dialog(f, dialog); }  // overlays last
    if let Some(t) = &state.toast { render_toast(f, t); }
}
```

Why this is fast enough without OpenTUI's scroll regions:

- ratatui diffs front/back buffers cell-by-cell — an unchanged transcript costs
  ~zero PTY bytes per frame.
- `committed` lines are `Line<'static>` built **once at commit time** (markdown →
  spans, syntect highlight, wrap cached). The per-frame work is only
  *slicing the visible window*, not re-parsing.
- Only `active` (the streaming message) is re-rendered per frame — exactly like
  OpenCode's retained surface.

Stick to the **main screen** (no alternate screen) for the direct-mode feel:
committed output naturally becomes the user's terminal scrollback on exit —
same as opencode's `clearOnShutdown: false`.

(Option B, closer to OpenTUI: use crossterm raw writes into a DECSTBM scroll region
for committed lines and ratatui only for the footer. More faithful, much fiddlier.
Don't start here.)

---

## 4. Streaming Markdown (the retained surface)

```rust
struct ActiveEntry {
    raw: String,                 // append-only source of truth
    rendered_height: usize,      // cached
}

fn on_delta(state: &mut AppState, delta: &str) {
    let entry = state.transcript.active.get_or_insert_with(ActiveEntry::new);
    entry.raw.push_str(delta);
    state.dirty = true;
}
```

Per frame, render `active.raw` through your existing `src/render/markdown.rs` into
`Vec<Line>`, append it after the committed lines, and scroll-pin to bottom unless
the user scrolled up. When `BackendEvent::Done` arrives:

1. Final render of `active.raw` → push onto `committed`.
2. `active = None`.

That *is* the commit step — `entryDone()` in their `scrollback.surface.ts`.

Performance guardrails (from their playbook):
- Re-highlight/re-wrap only the **tail block** of the streaming markdown, not the
  whole document (their `_stableBlockCount` trick). Simple version: split on the
  last blank line; cache everything before it.
- Word-wrap always; never horizontal-scroll the transcript.

---

## 5. Backend Channel Shape

rustchat already has the right shape — keep it:

```rust
enum BackendEvent {            // src/tui/event.rs (exists)
    StreamDelta(String),
    ToolCall(ToolCall),
    Done { text: String },
    Error(String),
    Info(String),
    SessionUpdated,
    Exit,
}
```

Add one thing — **turn lifecycle** — mirroring their serial prompt queue
(`runtime.queue.ts`):

```rust
enum TurnState { Idle, Running { abort: AbortHandle } }
// submit prompt → Running; Done/Error → Idle; footer view is a function of TurnState.
// Ctrl-C #1: clear draft. Ctrl-C #2 while Running: abort.abort(). (two-press pattern)
```

Serial turns only. No concurrent prompts — that's where the simplicity lives.

---

## 6. Keymap, Modes, Commands

Their `@opentui/keymap` is a modal layer system. The simple Rust version:

```rust
enum Mode { Base, Insert, Modal }          // base = global, insert = prompt typing
struct ModeStack(Vec<Mode>);               // dialogs push Modal, pop on close

fn on_key(state: &mut AppState, key: KeyEvent) {
    let mode = state.modes.current();
    // 1. try modal bindings (if dialog open)
    // 2. try insert bindings (if prompt focused)
    // 3. fall through to base bindings
    if let Some(cmd) = state.keymap.lookup(mode, key) {
        state.run_command(cmd);
    }
}
```

Commands as data (their `appCommands` list, `app.tsx:559-959`):

```rust
struct Command {
    name: &'static str,              // "session.list"
    title: &'static str,             // "Switch session"
    slash: Option<&'static str>,     // "sessions"
    aliases: &'static [&'static str],
    category: &'static str,
    run: fn(&mut AppState, &mut dyn Backend),
}
static COMMANDS: &[Command] = &[ /* ... */ ];
```

The command palette is then just: fuzzy-filter `COMMANDS` by query (use
`fuzzy-matcher`, already a dep — their fuzzysort equivalent) + frecency bump from a
small `HashMap<&str, f64>` persisted in `kv.rs`. That's the whole palette.

Default bindings worth stealing verbatim: `ctrl+p` palette, `ctrl+l` model list,
`tab` agent cycle, `esc` pop dialog, `ctrl+c` two-press (§5), `up/down` prompt
history, `/` opens slash autocomplete, `@` opens file autocomplete.

Leader keys / sequences: skip for v1. If wanted later, keep a
`Option<(KeyEvent, Instant)>` pending key and a 1s timeout in the tick.

---

## 7. Theme System (load OpenCode's own themes)

Their theme files (`packages/tui/src/theme/assets/*.json`) use: `defs` (named
colors), per-key `{dark, light}` variants, and string refs into defs. This deserializes
directly:

```rust
#[derive(Deserialize)]
struct ThemeJson {
    defs: Option<HashMap<String, String>>,
    theme: HashMap<String, ColorValue>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ColorValue {
    Ref(String),                    // "#fab283" or "darkStep9"
    Variant { dark: String, light: String },
}

fn resolve(json: &ThemeJson, mode: Mode, key: &str) -> Color {
    match &json.theme[key] {
        ColorValue::Ref(s) => resolve_ref(json, mode, s),
        ColorValue::Variant { dark, light } => {
            resolve_ref(json, mode, if mode == Dark { dark } else { light })
        }
    }
}
// resolve_ref: "#rrggbb" → parse; else look up defs (with a visited-set against
// circular refs — they throw on cycles too, theme/index.ts:250)
```

Map the resolved keys into a flat `Theme` struct with the same field names
(`primary`, `textMuted`, `backgroundPanel`, `diffAddedBg`, `markdownHeading`,
`syntaxKeyword`, …). Then:

- `Theme::detect()` (exists as a stub): use **terminal-colorsaurus** `theme_mode()`
  for dark/light — the analog of their `waitForThemeMode(1000)` luminance check.
  On failure, default dark (they default `"dark"` too, `app.tsx:242`).
- Ship `opencode.json` as the built-in default via `rust-embed` (already a dep).
- Custom themes: scan `~/.config/rustchat/themes/*.json` + `./.rustchat/themes/*.json`
  walking up from cwd — mirrors their `discoverThemes` (`context/theme.tsx:52-61`).
- "system" theme (generate grays from the terminal palette) is a nice-to-have;
  colorsaurus can also read the bg color. Defer.

---

## 8. Dialogs, Toasts, Overlays

No z-index needed — render order is the z-order. Draw base → dialogs → toast.

```rust
enum DialogKind { SessionList, ModelList, Palette, Confirm(String), Help }

fn render_dialog(f: &mut Frame, kind: &DialogKind) {
    // scrim: dim everything below (their RGBA(0,0,0,150))
    f.render_widget(Block::default().style(Style::default().bg(Color::Black).dim()), f.area());
    let area = centered(f.area(), 60);        // widths 60/88/116 by size
    f.render_widget(Clear, area);             // punch a hole in the scrim
    // panel: bg = theme.background_panel, padding via inner margin, SplitBorder rails
}
```

- **Focus**: `Mode::Modal` on the stack captures all keys; popping restores `Base`.
  That's their focus save/restore (`dialog.tsx:87-103`) in one enum.
- **Esc/Ctrl-C pops the top dialog** — handled in the modal binding layer only.
- **Toast**: `Option<Toast>` in state with `shown_at: Instant`; tick checks
  `elapsed > duration` → clear + dirty. Top-right `Rect` anchored at `(w-62, 2)`,
  `┃`-only border set, border color by variant. See DESIGN.md §4 for geometry.
- **SplitBorder**: ratatui `border_set!` / `BorderSet` with only left/right `┃`
  verticals — or simpler, a 1-wide colored `Block` column on each side.

---

## 9. The Scanner Spinner

Port of `ui/spinner.ts` (~30 lines of logic):

```rust
const SHAPES: [char; 4] = ['⬥', '◆', '⬩', '⬪'];
fn scanner_frame(frame: usize, width: usize) -> Vec<(char, f32)> {
    // bidirectional position with holds: fwd(width) + hold_end(9) + back(width-1) + hold_start(30)
    let (hold_start, hold_end) = (30, 9);
    let period = width + hold_end + (width - 1) + hold_start;
    let f = frame % period;
    let pos = if f < width { f }
              else if f < width + hold_end { width - 1 }                       // hold at end
              else if f < width + hold_end + width - 1 { width - 2 - (f - width - hold_end) }
              else { 0 };                                                     // hold at start
    (0..width).map(|i| {
        let d = pos as i32 - i as i32;               // directional distance (trail behind)
        let (ch, alpha) = match d {
            0 => (SHAPES[0], 1.0),
            1 => (SHAPES[1], 0.9),                   // bloom step
            2..=5 => (SHAPES[(d as usize).min(3)], 0.65f32.powi(d - 1)),
            _ => ('·', 0.2),                          // inactive
        };
        (ch, alpha)
    }).collect()
}
```

Render each cell with `fg = lerp(theme.background, theme.primary, alpha)` —
alpha-via-lerp because terminals have no real alpha. Tick increments `frame`.

---

## 10. Build Order (milestones)

| Milestone | Scope | Done when |
|---|---|---|
| **M0 — skeleton** | Terminal guard, event loop, two-chunk layout, echo prompt → transcript | type + submit, scrolling transcript, clean exit |
| **M1 — streaming** | Backend mpsc, coalescing drain, `ActiveEntry` retained buffer, markdown render, 30fps tick | tokens appear live at constant CPU |
| **M2 — interactivity** | Mode stack, base/insert bindings, two-press Ctrl-C, prompt history, `/` + `@` autocomplete | drives like opencode |
| **M3 — chrome** | Palette (fuzzy + frecency), dialogs (session/model list, confirm), toasts, scanner spinner, status line | feels like opencode |
| **M4 — themes** | ThemeJson loader, `opencode` default, dark/light detect, custom theme dirs | `/themes` dialog swaps palette live |

Each milestone is independently shippable; M0+M1 already deliver the "fast" feel.

## 11. Pitfalls (learned from their code)

- **Restore the terminal on panic** — install a panic hook that leaves raw mode /
  shows the cursor before printing the panic (their `destroyRenderer` + SIGHUP path).
- **Never block the loop** on I/O — everything arrives over channels; the loop only
  reduces + draws.
- **Cap the transcript** if sessions get huge (e.g. keep last N committed entries
  rendered; older ones as plain text). Their answer is native scroll regions; the
  ratatui answer is bounding retained render data.
- **Resize is the only full re-wrap** — cache wrapped lines keyed by width.
- **Terminal titles / palette queries** write escape sequences to stdout — keep them
  off the hot path (they prewarm the palette before mount for this reason,
  `app.tsx:241`).

---

## 12. Reference Reading (upstream)

| File | Why |
|---|---|
| `packages/opencode/src/cli/cmd/run/runtime.lifecycle.ts` | renderer options, shutdown order, two-press SIGINT, editor suspend |
| `packages/opencode/src/cli/cmd/run/scrollback.surface.ts` | retained streaming + block commits |
| `packages/opencode/src/cli/cmd/run/runtime.queue.ts` | serial turn loop |
| `packages/tui/src/context/sdk.tsx` | 16ms event coalescing |
| `packages/tui/src/context/sync.tsx` | central store shape |
| `packages/tui/src/keymap.tsx` | mode stack, command registry |
| `packages/tui/src/ui/dialog.tsx` / `toast.tsx` / `spinner.ts` | overlay geometry + scanner |
| `packages/tui/src/theme/index.ts` + `assets/opencode.json` | theme format + default palette |

