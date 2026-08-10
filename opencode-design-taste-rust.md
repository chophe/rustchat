# OpenCode TUI Design Taste — Extracted Principles & Rust Implementation Guide

> `/stitch-design-taste` / `/stitch-extract-design-md`
> Target: `/Users/ali/dev/rust/rustchat/my-fork/rustchat/`
> Companion doc: `opencode-tui-architecture.md` (file-by-file map of OpenCode's TUI code).

## What Is "Design Taste"?

Design taste is the **coherent set of architectural decisions** that make a system feel
right. It's not about individual features — it's about the patterns that recur, the
constraints that are enforced, and the things that are deliberately *not* done.

OpenCode's TUI has extremely high design taste. Here are the extracted principles, ranked
by importance, with concrete patterns you can implement in Rust.

---

## Principle 1: Split the Screen Into Immutable and Mutable Regions

**The rule:** One region is append-only (scrollback), one region is reactive (footer).
Never mix update models in the same region.

**Why it's tasteful:**
- Eliminates the O(n) re-render problem as scrollback grows
- The footer (prompt, status, dialogs) is what actually needs reactivity
- The scrollback is a log — it only grows, it never changes

**Rust implementation: ratatui, two Layout splits**

```rust
use ratatui::layout::{Constraint, Direction, Layout, Rect};

const FOOTER_ROWS: u16 = 4;

fn split_frame(area: Rect) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),             // scrollback — fills remaining space
            Constraint::Length(FOOTER_ROWS), // footer — fixed height
        ])
        .split(area);
    (chunks[0], chunks[1])
}
```

**What this unlocks:**
- The scrollback render function can be a simple "draw visible lines from a `Vec<String>`
  buffer" — no diffing, no invalidation tracking
- The footer render function is a match on the current footer mode (Prompt, Permission,
  Question, Status) — lightweight, always fits in 4 rows
- Frame budget: scrollback draws cheap (just slice iteration), footer draws cheap (tiny
  fixed area)

**What you should NOT do:**
- Don't put the prompt *inside* the scrollback area (that's what most TUI apps do, and
  it's why they feel sluggish — every keystroke triggers a full re-layout)
- Don't try to "diff" the scrollback — just append and let the renderer clip

---

## Principle 2: Streaming = In-Place Updates, Not Recreation

**The rule:** When LLM tokens arrive one by one, update a single retained renderable
in-place. Don't create a new widget for each chunk.

**Why it's tasteful:**
- Creating/destroying widgets per token is O(tokens) allocations and layout passes
- In-place updates let the layout engine reuse its computed geometry
- For markdown, you can do block-level incremental re-layout (only changed blocks)

**OpenCode does this with:**
```ts
// A single MarkdownRenderable created once at stream start
this.active = {
  renderable: new MarkdownRenderable({ content: "" }),
  surface: renderer.createScrollbackSurface(),
  content: "",
}

// Each token chunk:
this.active.content += newText
this.active.renderable.setText(this.active.content)  // in-place
this.active.surface.commitRows(start, end)            // only changed rows
```

**Rust implementation: stateful streaming widget**

```rust
/// A streaming text block that accumulates content and efficiently re-renders
struct StreamingBlock {
    /// Accumulated markdown content
    content: String,
    /// Cached rendered line count from last frame
    cached_line_count: u16,
    /// Whether content has changed since last render
    dirty: bool,
}

impl StreamingBlock {
    fn append(&mut self, chunk: &str) {
        self.content.push_str(chunk);
        self.dirty = true;
    }

    /// Render to a Ratatui Paragraph. The Paragraph widget handles its own
    /// wrapping/caching, so this is efficient.
    fn render<'a>(&'a mut self) -> Paragraph<'a> {
        self.dirty = false;
        Paragraph::new(self.content.as_str())
            .wrap(Wrap { trim: false })
    }
}
```

**Key insight:** Ratatui's `Paragraph` already caches its layout. If you reuse the same
`Paragraph` (or recreate it with the same text), the wrapping is computed once. The
important thing is to **not allocate a new string** for every append — accumulate into a
`String` buffer.

**What you should NOT do:**
- Don't keep a `Vec<TextChunk>` and iterate to build the paragraph each frame
- Don't create a new widget tree for each token
- Don't re-parse markdown from scratch every frame — cache the parsed AST

---

## Principle 3: Events Flow Through a Single Pipe

**The rule:** All external events (LLM tokens, permission requests, status changes) flow
through a single event enum. The main loop pattern-matches on events and updates state.
Components don't subscribe to individual event sources.

**Why it's tasteful:**
- One `select!` / `tokio::select!` in the main loop, not scattered subscriptions
- Easy to reason about ordering — events are processed sequentially
- Easy to test — pump events into the loop and assert state transitions

**Rust implementation: unified event enum**

```rust
use tokio::sync::mpsc;

#[derive(Debug)]
enum AppEvent {
    /// User keyboard input
    Key(crossterm::event::KeyEvent),
    /// Terminal resize
    Resize(u16, u16),
    /// Mouse event
    Mouse(crossterm::event::MouseEvent),
    /// SSE/WebSocket event from the backend
    Backend(BackendEvent),
    /// Internal tick (for animations, debounce)
    Tick,
}

#[derive(Debug)]
enum BackendEvent {
    MessageDelta { session_id: String, message_id: String, delta: String },
    MessageCompleted { session_id: String, message_id: String },
    ToolCall { session_id: String, tool: String, input: serde_json::Value },
    ToolResult { session_id: String, call_id: String, output: String },
    PermissionRequest { session_id: String, request_id: String, kind: String },
    SessionStatus { session_id: String, status: SessionStatus },
}
```

**Main loop structure:**

```rust
async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
    mut events: mpsc::UnboundedReceiver<AppEvent>,
    mut backend_rx: mpsc::UnboundedReceiver<BackendEvent>,
) -> Result<()> {
    // Spawn a thread for crossterm input events
    let mut interval = tokio::time::interval(Duration::from_millis(33)); // ~30fps

    loop {
        tokio::select! {
            Some(event) = events.recv() => {
                if !state.handle_event(event) { break; }
            }
            Some(backend_event) = backend_rx.recv() => {
                state.handle_backend_event(backend_event);
            }
            _ = interval.tick() => {
                state.handle_tick();
            }
        }
        terminal.draw(|frame| state.render(frame))?;
        if state.should_exit() { break; }
    }
    Ok(())
}
```

**What makes this different from typical Rust TUI loops:**
- Uses `tokio::select!` to merge crossterm input + backend events + tick into one loop
- The `interval.tick()` at ~30fps drives animations and ensures the terminal gets
  refreshed even without input events
- `terminal.draw()` runs on every iteration — ratatui is diff-based internally, so this
  is cheap when nothing changes

---

## Principle 4: State as a Single Struct, Not Scattered Mutable Variables

**The rule:** All application state lives in one `AppState` struct. Components read from
it; event handlers mutate it. No component-local state for shared concerns.

**Why it's tasteful:**
- You can snapshot the entire state for debugging
- State transitions are explicit — you can trace every mutation
- No stale references or out-of-sync component state

**Rust implementation:**

```rust
struct AppState {
    // Navigation
    route: Route,

    // Session data
    sessions: Vec<SessionMeta>,
    current_session: Option<SessionState>,

    // Streaming state
    streaming_block: Option<StreamingBlock>,

    // Footer state
    footer_mode: FooterMode,
    status_message: String,
    pending_permissions: Vec<PermissionRequest>,

    // Scrollback
    scrollback: Vec<ScrollbackEntry>,
    scroll_offset: usize,

    // User preferences
    selected_agent: String,
    selected_model: Option<ModelRef>,
    sidebar_open: bool,

    // Theme
    theme: Theme,

    // Exit flag
    exit: bool,
}

enum Route {
    Home,
    Session(String), // session ID
}

enum FooterMode {
    Prompt { draft: String, cursor: usize },
    Permission(PermissionRequest),
    Question(String, Vec<String>),
    Status(String),
    Hidden,
}

enum ScrollbackEntry {
    UserMessage { text: String },
    AssistantStream { block: StreamingBlock },
    AssistantComplete { text: String },
    ToolCall { tool: String, status: String },
    ToolOutput { output: String },
    Separator,
}
```

**Key design choices:**
- `StreamingBlock` is stored **in** the scrollback vec, not alongside it — it's the
  last entry while streaming is active
- When streaming completes, the `StreamingBlock` is converted to `AssistantComplete`
- The scrollback is just a `Vec` — no widget tree, no layout cache. Ratatui handles
  wrapping/rendering per-frame.

---

## Principle 5: The Footer Is a State Machine

**The rule:** The footer has exactly one "mode" at a time. Changing the mode is a state
transition. The render function is a simple match.

**Why it's tasteful:**
- No z-ordering or overlay logic in the renderer
- The footer is small (4 rows) so mode transitions are cheap
- Each mode has a clear lifecycle: enter → render → exit

**Rust implementation:**

```rust
impl AppState {
    fn render_footer(&self, area: Rect, frame: &mut Frame) {
        match &self.footer_mode {
            FooterMode::Prompt { draft, cursor } => {
                self.render_prompt_footer(area, frame, draft, *cursor);
            }
            FooterMode::Permission(req) => {
                self.render_permission_footer(area, frame, req);
            }
            FooterMode::Question(header, options) => {
                self.render_question_footer(area, frame, header, options);
            }
            FooterMode::Status(msg) => {
                self.render_status_footer(area, frame, msg);
            }
            FooterMode::Hidden => {}
        }
    }
}
```

The footer renders into 4 rows:

```
Row 1: Status bar (agent • model • connection status)
Row 2: Main content (prompt input / permission text / question)
Row 3: Secondary content (hints, shortcuts, progress)
Row 4: Keybinding hints (^C Exit  ^S Submit  ^E Editor  ...)
```

---

## Principle 6: Autocomplete As a Separate Reactive Surface

**The rule:** The autocomplete dropdown is not a modal dialog. It's a floating panel that
appears above the prompt and is driven by the same prompt state.

**Why it's tasteful:**
- No dialog stack management for autocomplete — it's just "show if there are matches"
- The list updates reactively as you type
- Selection state is local to the autocomplete widget

**Rust implementation:**

```rust
struct AutocompleteState {
    /// Candidates matching current input prefix
    candidates: Vec<String>,
    /// Currently selected index (0-based, cycles with Tab)
    selected: usize,
    /// The prefix these candidates match
    prefix: String,
}

impl AutocompleteState {
    fn update(&mut self, current_word: &str) {
        if current_word != self.prefix {
            self.prefix = current_word.to_string();
            self.candidates = find_matches(current_word); // files, commands, agents
            self.selected = 0;
        }
    }

    fn next(&mut self) {
        if !self.candidates.is_empty() {
            self.selected = (self.selected + 1) % self.candidates.len();
        }
    }

    fn previous(&mut self) {
        if !self.candidates.is_empty() {
            self.selected = self.selected.checked_sub(1)
                .unwrap_or(self.candidates.len() - 1);
        }
    }

    fn accept(&self) -> Option<&str> {
        self.candidates.get(self.selected).map(|s| s.as_str())
    }
}
```

---

## Principle 7: Prompt Submission Is Fire-and-Forget (With Blocking Wait)

**The rule:** Submitting a prompt does NOT start a new async task that races with the UI.
The prompt submission function is async and blocks the main loop until the turn completes
(session becomes idle). During the wait, backend events continue to arrive and update the
UI.

**Why it's tasteful:**
- No concurrent turn management — one turn at a time
- The prompt input is disabled during streaming (FooterMode::Status)
- Interrupt is a first-class action: send abort, flush partial results

**Rust implementation:**

```rust
impl AppState {
    async fn submit_prompt(
        &mut self,
        client: &Client,
        prompt: String,
    ) -> Result<()> {
        // 1. Append user message to scrollback
        self.scrollback.push(ScrollbackEntry::UserMessage {
            text: prompt.clone(),
        });

        // 2. Switch footer to status mode (hides prompt)
        self.footer_mode = FooterMode::Status("Thinking...".into());

        // 3. Create streaming block for assistant response
        let block = StreamingBlock::default();
        self.scrollback.push(ScrollbackEntry::AssistantStream {
            block: block.clone(), // Arc<Mutex<>> for shared access
        });

        // 4. Send prompt to backend — this blocks until the session is idle
        //    OR until interrupted. Backend events continue to arrive on the
        //    channel and are processed between .await points.
        let result = client
            .send_prompt(&self.current_session_id, &prompt)
            .await;

        // 5. Finalize streaming block
        if let Some(ScrollbackEntry::AssistantStream { block }) = self.scrollback.last_mut() {
            let content = block.lock().unwrap().content.clone();
            *self.scrollback.last_mut().unwrap() = ScrollbackEntry::AssistantComplete {
                text: content,
            };
        }

        // 6. Restore prompt mode
        self.footer_mode = FooterMode::Prompt {
            draft: String::new(),
            cursor: 0,
        };

        result
    }
}
```

---

## Principle 8: Dialogs Are a Stack, Not Conditional Rendering

**The rule:** Modal dialogs live on a stack. Only the top dialog is rendered (covering
the full screen or centered). Dismissing pops the stack.

**Why it's tasteful:**
- Clean separation: dialogs don't need to know about each other
- Keyboard events propagate top-down: top dialog handles first, then pass through
- Stack-based z-ordering is trivial to implement

**Rust implementation (simplified):**

```rust
enum Dialog {
    SessionList,
    ModelPicker,
    AgentPicker,
    Confirm { message: String },
    Alert { message: String },
}

struct DialogStack(Vec<Dialog>);

impl DialogStack {
    fn push(&mut self, dialog: Dialog) { self.0.push(dialog); }

    fn pop(&mut self) -> Option<Dialog> { self.0.pop() }

    fn top(&self) -> Option<&Dialog> { self.0.last() }

    /// Returns true if the key was handled by a dialog
    fn handle_key(&mut self, key: KeyEvent, state: &mut AppState) -> bool {
        match self.top() {
            Some(Dialog::Confirm { .. }) if key.code == KeyCode::Enter => {
                self.pop();
                true // handled
            }
            Some(_) if key.code == KeyCode::Esc => {
                self.pop();
                true // handled — dismiss top dialog
            }
            _ => false, // pass through to main handler
        }
    }

    /// Renders the top dialog (or nothing if stack is empty)
    fn render(&self, area: Rect, frame: &mut Frame) {
        match self.top() {
            Some(Dialog::Confirm { message }) => {
                // Render centered confirm dialog
            }
            Some(Dialog::Alert { message }) => {
                // Render centered alert dialog
            }
            None => {} // No dialog visible
        }
    }
}
```

---

## How to Clone This in Rust — The Practical Path

Your existing codebase at `src/tui/` already has:
- `app.rs` — main loop with `crossterm` + `ratatui`, input thread, backend channel
- `state.rs` — centralized `AppState`
- `backend/` — embedded and remote backends
- `widgets/` — dialog, message_list, prompt, sidebar, status_bar, toast
- `routes/` — home, session
- `theme.rs` — theme detection

This is a solid foundation. Here's what to change/add to match OpenCode's design taste:

### Phase 1: Split-Footer Layout (highest impact, one file change)

1. **Modify your render function** in `app.rs` to split the frame into scrollback + footer
   regions using `Layout::vertical()`
2. **Move the prompt from inline (in scrollback) to the footer** — this is the single
   biggest UX improvement. Currently your `widgets/prompt.rs` renders in the main area.
   Move it to the bottom 4 rows.
3. **Make the footer region fixed at 4 rows** with structure: status bar, prompt,
   hints, keybinding bar

### Phase 2: Streaming Scrollback

1. **Create a `StreamingBlock` struct** that accumulates text in-place (in a new
   `scrollback.rs` module)
2. **When a backend delta arrives**, append to the streaming block instead of creating a
   new `Paragraph` widget
3. **When the message completes**, finalize the block into a static entry

### Phase 3: Footer as State Machine

1. **Define `FooterMode` enum** (Prompt, Permission, Question, Status) — this replaces
   your current split of prompt/status_bar widgets
2. **Replace scattered footer widgets** with a single `render_footer()` function in
   a new `footer.rs` that matches on mode
3. **Add prompt submission lifecycle**: Prompt → Status → Prompt

### Phase 4: Autocomplete

1. **Extract `AutocompleteState`** from the prompt widget
2. **Implement file/command completion** as a floating list above the prompt
3. **Wire Tab/Shift-Tab** for cycling through candidates

### Phase 5: Dialog Stack

1. **Replace `DialogKind` enum** with a `DialogStack(Vec<Dialog>)` in `widgets/dialog.rs`
2. **Implement stack-based event routing**: top dialog gets first crack at keys
3. **Add Confirm dialog type** for interrupt/exit two-press pattern (Ctrl-C twice to
   exit)

---

## What NOT to Clone

Some things in OpenCode are over-engineered for a simpler Rust clone:

- **Effect-based service layers** — You don't need Effect-TS's dependency injection in
  Rust. A simple `Arc<AppState>` or channel-based message passing is sufficient.
- **Plugin system** — OpenCode has a full plugin runtime. Skip this unless you need
  extensions.
- **Subagent tabs** — The subagent footer tabs are clever but complex. Start with
  single-session.
- **The 20-layer context provider tree** — This is a JS/SolidJS pattern. In Rust, a
  single `AppState` struct with methods is cleaner.
- **The keymap DSL** — OpenCode has a custom binding language with modes, layers, and
  leader keys. For a Rust clone, use simple match statements or a
  `HashMap<String, KeyBinding>`.
- **Tree-sitter syntax highlighting in scrollback** — This is impressive but expensive.
  Start with plain text or `syntect`-based highlighting as a stretch goal.
- **Split-footer + capture-stdout for external processes** — OpenCode captures
  subprocess output into its scrollback pipeline. This is niche. Skip for now.

---

## Summary: The 8 Design Principles

| # | Principle | One-sentence takeaway |
|---|---|---|
| 1 | Split screen | Scrollback is append-only, footer is reactive. Never mix. |
| 2 | In-place streaming | Update one retained widget per token, don't recreate. |
| 3 | Single event pipe | One `select!` loop, one event enum, sequential processing. |
| 4 | Centralized state | One struct owns all state. Components are pure functions of state. |
| 5 | Footer state machine | Footer has modes, not conditional widget trees. |
| 6 | Reactive autocomplete | Floating list driven by prompt state, not a dialog. |
| 7 | Serial turns | One prompt at a time, block until idle, events still flow. |
| 8 | Dialog stack | Dialogs are a stack. Top handles keys first. Pass-through otherwise. |

**The most impactful single change for RustChat:** implement the split-footer layout.
Everything else builds on that foundation. Once the prompt lives in a fixed 4-row footer
and the scrollback is append-only, you'll feel the difference immediately — the UI stops
getting slower as the conversation grows.
