# Agent Task Prompt: Implement the OpenCode-style TUI in rustchat

> Give this file to a coding agent working inside the rustchat repository.
> It is self-contained: read it, read the two reference files, then implement.

---

## Role

You are an experienced Rust TUI engineer working on the `rustchat` codebase
(`/Users/ali/dev/rust/rustchat/my-fork/rustchat`). Your mission is to upgrade the
existing interactive TUI so it feels as fast and as polished as OpenCode's terminal UI —
using the architectural analysis already produced for this repo.

## Read These Files First (in order)

1. `opencode-tui-architecture.md` (repo root) — how OpenCode's TUI is actually built:
   the split-footer renderer, the append-only scrollback, the event pipeline, the
   streaming model.
2. `opencode-design-taste-rust.md` (repo root) — the 8 extracted design principles with
   ready-made Rust sketches for each one. This is your implementation playbook.
   (Optional: `opencode-tui-how-it-works.md` is a third-party companion write-up with
   version-specific detail; skim it if present, but treat it as secondary.)

Then read the current implementation:

3. `src/tui/app.rs` — main loop (`crossterm` + `ratatui`), input thread, backend channel,
   `tokio`-driven run loop.
4. `src/tui/state.rs` — `AppState` struct, `map_key`, backend event handling.
5. `src/tui/event.rs` — `AppEvent`, `BackendEvent`, `Focus`, `KeyAction`.
6. `src/tui/widgets/mod.rs` — the layout + `render()` entry point.
7. `src/tui/widgets/message_list.rs`, `prompt.rs`, `dialog.rs`, `status_bar.rs` — the
   current widgets.
8. `src/tui/backend/mod.rs` — the `ChatBackend` trait (`embedded.rs`, `remote.rs`).

## Ground Truth: What Already Exists

The current TUI is roughly 40% of the way to the OpenCode design. Do not rebuild it from
scratch — close the gap. Facts you must respect:

- **Stack**: `ratatui 0.29` (default-features off, crossterm feature), `crossterm 0.28.1`,
  `tui-textarea 0.7`, `syntect 5.0` (highlighting available), `fuzzy-matcher 0.3.7`
  (available for autocomplete), `terminal-colorsaurus 0.4.8` (theme detection),
  `arboard` (clipboard), `tokio`. See `Cargo.toml`.
- **Launch**: `rustchat --tui` → `WorkingMode::Tui` → `tui::run(config, tui_url)` in
  `src/main.rs`. Keep this entry point working.
- **Layout already split** (widgets/mod.rs): horizontal sidebar split, then vertical
  `[header(1), messages(Min(3)), status_bar(1), prompt(4)]`. The prompt is already in a
  fixed bottom region — good, keep that.
- **Streaming state**: `BackendEvent::StreamDelta(String)` → `AppState::append_stream_delta`
  appends in-place to the last `MessageView.text` (in-place string growth — correct).
- **The core performance problem to fix**: `render_messages` rebuilds a `Vec<Line>` for
  the ENTIRE transcript on every frame. As the conversation grows, every streamed token
  re-wraps everything above it. This is the O(n) re-render issue the analysis warns about.
- **Events**: `BackendEvent` has `StreamDelta | ToolCall | Done{text} | Error | Info |
  SessionUpdated | Exit`. There is no `PermissionRequest` or `Question` variant yet.
- **Dialog**: `Option<DialogKind>` — a single dialog, not a stack.
- **Key handling**: a `map_key` match table in `state.rs` producing `KeyAction`.
- **There are existing tests**: `cargo test` must keep passing. Run tests from the
  package root (`/Users/ali/dev/rust/rustchat/my-fork/rustchat`), not a subdirectory.

## Mission / Definition of Done

Deliver a TUI that:

1. Stays smooth while streaming long responses — per-token cost must NOT grow with
   transcript length (no full re-wrap of history per frame).
2. Shows the OpenCode-style four-zone footer: status line, prompt, hints, keybinding bar —
   with the prompt always visible and interactive during streaming (you can type the next
   prompt while the model finishes; it queues).
3. Streams markdown-aware text in place: a streaming block that re-lays-out only itself.
4. Supports the essential interactive surface: command palette, autocomplete (files +
   commands), dialog stack (modal on top, Esc to close), toast notifications, mouse
   scroll on macOS-style acceleration where enabled.
5. Two-press interrupt: first Esc/Ctrl-C during streaming shows a hint; the second press
   (within a window) aborts.

## Implementation Phases (do them in order; each phase compiles and works before the next)

### Phase 1 — Split-Footer Layout + Incremental Streaming (highest impact)

Goal: eliminate per-frame O(transcript) re-layout.

1. In `src/tui/widgets/mod.rs`, keep the current zone layout but make the messages zone
   an **append-only log with an incremental line cache**:
   - Introduce a `Scrollback` structure (new module `src/tui/scrollback.rs`) that owns
     `entries: Vec<ScrollbackEntry>` plus a cached `Vec<Line>` built incrementally.
   - `ScrollbackEntry::Streaming { text: String }` is the last entry while a delta is
     arriving; appending to it must only append lines to the cache (or mark a
     "tail" region dirty), never rebuild earlier entries.
   - When `Done{text}` arrives, finalize `Streaming` into a static entry and run markdown
     rendering through `src/render/markdown.rs` once (syntect is available for code
     blocks; keep it optional/behind a flag if it slows the frame).
   - Only the new/streamed lines are wrapped each frame; history lines are reused.
   - Keep `AppState::scroll` semantics working (scroll offset, auto-scroll to bottom on
     stream).
2. Split the footer into the four-row structure from `opencode-design-taste-rust.md`
   Principle 5: row 1 status (agent • model • connection), row 2 prompt/main content,
   row 3 hints/secondary, row 4 keybinding bar. `status_bar.rs` and `prompt.rs` already
   cover rows 1–2; add rows 3–4 (a `hints.rs` widget).
3. Acceptance: `cargo build` clean; manually verify `rustchat --tui` streams a response
   with visibly constant frame cost as the transcript grows; `cargo test` passes.

### Phase 2 — Footer as a State Machine

Goal: the footer has exactly one mode at a time.

1. Add `FooterMode` enum in `state.rs` (or `src/tui/footer.rs`):
   `Prompt { draft, cursor } | Streaming { status } | Permission(..) | Question(..) |
   Hidden`.
2. `render_footer` matches on the mode; the prompt widget is hidden (or replaced by a
   progress line) while `Streaming`. This mirrors OpenCode: while a turn runs, the input
   area shows status, and the prompt returns when the session is idle.
3. While streaming, keep accepting typed text into a **queued draft** so the user can
   compose the next prompt mid-turn (OpenCode keeps the prompt interactive).
4. Acceptance: switching prompt→streaming→prompt is visually clean; typing during
   streaming is buffered, not lost.

### Phase 3 — Single Event Pipe & Serial Turns

Goal: one select-loop, one event enum, sequential prompt turns.

1. In `src/tui/app.rs`, restructure the run loop around `tokio::select!` merging:
   `crossterm` input events, backend channel events, and a ~30fps `interval.tick()` for
   animations/toasts (see Principle 3 sketch). Render after every iteration — ratatui's
   internal diff makes no-op frames cheap.
2. Make prompt submission **serial**: submitting a prompt sets footer to `Streaming`,
   sends the request, and the loop continues processing backend deltas until `Done` (or
   `Error`) arrives — then returns the footer to `Prompt` and flushes the queued draft.
   Add an `Abort` path that sends a cancel and finalizes the partial stream (like
   `flushInterrupted` in OpenCode).
3. Add `PermissionRequest` and `Question` variants to `BackendEvent` (check
   `src/tui/backend/*` to see what the backends can already emit; if nothing emits them
   yet, define the enum variants + a `Permission` footer mode and wire the plumbing so a
   future backend can use them).
4. Acceptance: exactly one turn in flight; interrupts finalize partial text instead of
   dropping it; `cargo test` green.

### Phase 4 — Autocomplete + Command Palette polish

Goal: OpenCode-style prompt autocomplete as a floating panel above the prompt.

1. New `AutocompleteState` (Principle 6 sketch) driven by the current input word:
   candidates from files (use `fuzzy-matcher` + a scoped directory walk, debounced) and
   built-in commands (reuse `PaletteCommand::defaults()`).
2. Render the candidate list as an overlay just above the prompt row; Tab/Shift-Tab
   cycle, Enter accepts the highlighted candidate (only when the popup is open and the
   user pressed Tab to activate it — do not hijack Enter-to-submit).
3. Keep the palette (`Ctrl+P`) working; optionally move it onto the same overlay
   rendering path.
4. Acceptance: typing `/` or a word after space shows candidates; Tab cycles; Esc closes.

### Phase 5 — Dialog Stack + Two-Press Interrupt

Goal: modal dialogs on a stack; safe interrupt/exit.

1. Replace `Option<DialogKind>` with `DialogStack(Vec<Dialog>)` (Principle 8 sketch).
   Top dialog renders centered; Esc pops; keys route top-first, then fall through.
   Migrate all existing `open_dialog` / `close_dialog` call sites.
2. Two-press interrupt: `map_key` — during `Streaming`, first Esc sets
   `interrupt_armed: bool` + a toast hint ("Press Esc again to stop"); second Esc within
   ~5s (track via tick) sends `Abort`. Outside streaming, two Esc presses exit (first
   shows "Press Esc again to quit").
3. Acceptance: nested dialogs stack correctly; double-Esc pattern works with the
   documented timing; tests updated.

## Constraints & Guardrails

- **Keep everything working**: `cargo build`, `cargo clippy -- -D warnings` (or at least
  no new warnings), `cargo test` all green from the package root. Do not change
  `src/main.rs` entry behavior, the `ChatBackend` trait's public surface, or
  `BackendEvent`/`AppEvent` shapes without updating all consumers in the same change.
- **Minimal deps**: use what is already in `Cargo.toml`. Only add a new dependency if a
  phase genuinely requires it and justify it in the summary. Prefer `syntect` (already
  present) for code highlighting; `fuzzy-matcher` for autocomplete.
- **ratatui 0.29 API**: verify widget/layout API calls with `cargo check` — this is
  ratatui 0.29 (not 0.28 or 0.30); some APIs differ across versions.
- **Do not over-engineer**: skip OpenCode's plugin system, subagent tabs, keymap DSL,
  and 20-layer provider tree (see "What NOT to Clone" in the design-taste file). A single
  `AppState` struct + the phases above are the whole plan.
- **Preserve user-facing behavior**: existing keybindings (Ctrl+P palette, Ctrl+B
  sidebar, Ctrl+S sessions, Ctrl+M model, Ctrl+R role, Ctrl+H help, Ctrl+N home, Esc
  abort, j/k/arrows scroll) must keep working. Extend, don't break.
- **Testing**: add focused unit tests for new pure logic — scrollback incremental cache,
  footer mode transitions, dialog stack routing, autocomplete matching, two-press
  interrupt state machine. Existing test conventions in the repo apply.
- **Style**: match the existing code style in `src/tui/` (plain functions, no framework
  over-abstraction, `anyhow::Result` for fallible paths).

## Verification Steps (run before finishing)

1. `cargo build` (debug at minimum, `--release` if time permits)
2. `cargo clippy --all-targets` — no new warnings
3. `cargo test` — all pass
4. Manual smoke test (if a terminal is available): `rustchat --tui`, send a long
   prompt, verify smooth streaming, autocomplete, palette, dialog, interrupt, and that
   typing during streaming is queued and flushed after `Done`.

## Hand Back

In your final report, list:

- Every file changed/created (with one-line purpose each)
- Which phase each change belongs to, and phase acceptance results
- What was verified (build/clippy/test/smoke) and what was not
- Any deviations from this prompt and why
- The single next improvement you'd recommend

## Out of Scope (do not implement)

- OpenCode plugin runtime / TUI plugins
- Sub-agent tabs and multi-session tree UI
- External process output capture into scrollback
- A Vim-style keymap DSL with modes/layers/leader keys
- Split-window panes, mouse-resizable splits, terminal suspend (`Ctrl+Z`)
