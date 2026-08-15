use crate::client::MessageRole;
use crate::function::ToolCall;
use crate::render::{MarkdownRender, RenderOptions};
use crate::tui::state::MessageView;
use crate::tui::theme::{Theme, ThemeMode};

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Incremental append-only scrollback used to render the conversation log.
///
/// Design (mirrors OpenCode's split-footer scrollback):
/// - Static entries render their wrapped `Line`s once and never again.
/// - The single streaming entry appends raw text and only ever re-wraps its
///   own un-committed tail, so the per-token cost is bounded by the length of
///   the currently-open line, not by the length of the whole transcript.
/// - Markdown is rendered once per message when the stream finishes (via
///   `MarkdownRender`); it is never re-parsed on every frame.
pub struct Scrollback {
    entries: Vec<Entry>,
    total_lines: usize,
    cached_width: Option<usize>,
    /// Set when a structural change happened (new message, finalize, resize)
    /// so the next `prepare` rebuilds all cached lines.
    dirty: bool,
    markdown_enabled: bool,
    markdown: Option<MarkdownRender>,
    mode: ThemeMode,
}

impl Scrollback {
    pub fn new(mode: ThemeMode, markdown_enabled: bool) -> Self {
        Self {
            entries: vec![],
            total_lines: 0,
            cached_width: None,
            dirty: true,
            markdown_enabled,
            markdown: None,
            mode,
        }
    }

    pub fn line_count(&self) -> usize {
        self.total_lines
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.total_lines = 0;
        self.dirty = false;
    }

    /// Replace the whole log with messages loaded from a backend.
    pub fn load_messages(&mut self, _theme: &Theme, messages: Vec<MessageView>) {
        self.entries = messages
            .into_iter()
            .map(|m| Entry::from_view(m, _theme))
            .collect();
        self.dirty = true;
    }

    pub fn push_user(&mut self, _theme: &Theme, text: String) {
        self.entries.push(Entry::plain(
            MessageRole::User,
            text,
            false,
            vec![],
            _theme,
        ));
        self.dirty = true;
    }

    pub fn append_stream_delta(&mut self, _theme: &Theme, delta: &str) {
        if let Some(last) = self.entries.last_mut() {
            if last.streaming {
                last.text.push_str(delta);
                last.pending.push_str(delta);
                return;
            }
        }
        self.entries.push(Entry::plain(
            MessageRole::Assistant,
            delta.to_string(),
            true,
            vec![],
            _theme,
        ));
        self.dirty = true;
    }

    pub fn append_tool_call(&mut self, _theme: &Theme, call: ToolCall) {
        if let Some(last) = self.entries.last_mut() {
            last.tool_calls.push(call);
        }
        self.dirty = true;
    }

    /// Finalize a streamed message (called on `Done{text}`): run markdown
    /// rendering once and cache the styled lines.
    pub fn finish_stream(&mut self, _theme: &Theme, text: String) {
        if let Some(last) = self.entries.last_mut() {
            if last.streaming {
                last.streaming = false;
                last.text = text;
                last.pending.clear();
                last.partial_lines = 0;
            }
        }
        self.ensure_markdown();
        self.dirty = true;
    }

    /// Keep the partial streamed text as a completed plain message
    /// (called on abort) instead of dropping it.
    pub fn finalize_interrupt(&mut self, _theme: &Theme) {
        if let Some(last) = self.entries.last_mut() {
            if last.streaming {
                last.streaming = false;
                last.pending.clear();
                last.partial_lines = 0;
                last.rendered = None;
            }
        }
        self.dirty = true;
    }

    /// Cheap content check against a backend snapshot. Used to skip the
    /// O(history) line/markdown rebuild when nothing actually changed.
    pub fn same_as(&self, messages: &[MessageView]) -> bool {
        if self.entries.len() != messages.len() {
            return false;
        }
        self.entries.iter().zip(messages).all(|(e, m)| {
            e.role == m.role
                && e.text == m.text
                && e.streaming == m.streaming
                && tool_calls_eq(&e.tool_calls, &m.tool_calls)
        })
    }

    /// Ensure cached lines are current for the given width. Called every
    /// frame; when nothing structurally changed this only re-wraps the
    /// streaming tail.
    pub fn prepare(&mut self, theme: &Theme, width: usize) {
        let width = width.max(1);
        if self.dirty || self.cached_width != Some(width) {
            self.rebuild_all(theme, width);
            self.dirty = false;
            self.cached_width = Some(width);
            return;
        }
        if let Some(last) = self.entries.last_mut() {
            if last.streaming {
                last.flush_pending(theme, width, &mut self.total_lines);
            }
        }
    }

    fn rebuild_all(&mut self, theme: &Theme, width: usize) {
        let mut entries = std::mem::take(&mut self.entries);
        let mut total = 0usize;
        for entry in entries.iter_mut() {
            entry.lines.clear();
            entry.pending.clear();
            entry.partial_lines = 0;

            if entry.rendered.is_some() {
                if let Some(md) = self.markdown.as_mut() {
                    let out = md.render_reset(&entry.text, Some(width as u16));
                    entry.rendered = Some(out);
                }
            }
            if let Some(rendered) = &entry.rendered {
                entry.lines.push(entry.header_line(theme));
                total += 1;
                for raw in rendered.split('\n') {
                    entry.lines.push(ansi_to_line(raw, theme));
                    total += 1;
                }
            } else {
                entry.lines.push(entry.header_line(theme));
                total += 1;
                if entry.streaming {
                    // Streaming body lines are produced by flush_pending below
                    // so that only the un-committed tail is ever re-wrapped.
                    entry.pending = entry.text.clone();
                } else {
                    for raw in entry.text.split('\n') {
                        for wrapped in wrap_plain(raw, width) {
                            entry.lines.push(body_line(theme, wrapped));
                            total += 1;
                        }
                    }
                }
            }
            entry.push_tool_lines(theme, &mut total);
        }
        if let Some(last) = entries.last_mut() {
            if last.streaming {
                last.flush_pending(theme, width, &mut total);
            }
        }
        self.entries = entries;
        self.total_lines = total;
    }

    /// Return the cached lines for the visible window `[start, start+count)`.
    pub fn visible_lines(&self, start: usize, count: usize) -> Vec<&Line<'static>> {
        let mut out = Vec::new();
        let mut offset = 0usize;
        for entry in &self.entries {
            let n = entry.lines.len();
            if offset + n <= start {
                offset += n;
                continue;
            }
            let from = start.saturating_sub(offset);
            for line in entry.lines.iter().skip(from) {
                out.push(line);
                if out.len() >= count {
                    return out;
                }
            }
            offset += n;
        }
        out
    }

    fn ensure_markdown(&mut self) {
        if !self.markdown_enabled || self.markdown.is_some() {
            return;
        }
        let theme = syntect_theme(self.mode);
        let options = RenderOptions {
            theme,
            wrap: None,
            wrap_code: false,
            truecolor: true,
        };
        self.markdown = MarkdownRender::init(options).ok();
    }
}

fn tool_calls_eq(a: &[ToolCall], b: &[ToolCall]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| {
            x.name == y.name && x.arguments == y.arguments && x.id == y.id
        })
}

struct Entry {
    role: MessageRole,
    text: String,
    streaming: bool,
    tool_calls: Vec<ToolCall>,
    rendered: Option<String>,
    lines: Vec<Line<'static>>,
    pending: String,
    partial_lines: usize,
}

impl Entry {
    fn from_view(view: MessageView, theme: &Theme) -> Self {
        Self::plain(view.role, view.text, view.streaming, view.tool_calls, theme)
    }

    fn plain(role: MessageRole, text: String, streaming: bool, tool_calls: Vec<ToolCall>, _theme: &Theme) -> Self {
        Self {
            role,
            text,
            streaming,
            tool_calls,
            rendered: None,
            lines: vec![],
            pending: String::new(),
            partial_lines: 0,
        }
    }

    fn header_line(&self, theme: &Theme) -> Line<'static> {
        let style = match self.role {
            MessageRole::User => theme.user_label,
            MessageRole::Assistant => theme.assistant_label,
            MessageRole::System | MessageRole::Tool => theme.system_label,
        };
        let mut spans = vec![Span::styled(format!("[{}]", self.label()), style)];
        if self.streaming {
            spans.push(Span::styled(
                " ●",
                Style::default().fg(theme.accent),
            ));
        }
        Line::from(spans).style(Style::default().bg(theme.bg))
    }

    fn label(&self) -> &'static str {
        match self.role {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
            MessageRole::Tool => "tool",
        }
    }

    fn push_tool_lines(&mut self, theme: &Theme, total: &mut usize) {
        for tool in &self.tool_calls {
            self.lines.push(Line::from(Span::styled(
                format!("  tool: {}({})", tool.name, tool.arguments),
                Style::default().fg(theme.dim).bg(theme.bg),
            )));
            *total += 1;
        }
    }

    /// Consume `pending` text into wrapped lines. Complete lines (terminated
    /// by `\n`) are wrapped once; the trailing partial is re-wrapped only when
    /// it grows, after removing its previously cached lines.
    fn flush_pending(&mut self, theme: &Theme, width: usize, total: &mut usize) {
        if self.pending.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.pending);
        let has_partial = !pending.ends_with('\n');
        let (complete, partial) = if has_partial {
            match pending.rfind('\n') {
                Some(i) => (&pending[..i + 1], &pending[i + 1..]),
                None => ("", pending.as_str()),
            }
        } else {
            (pending.as_str(), "")
        };

        // Drop the previously cached partial lines (they will be rebuilt).
        while self.partial_lines > 0 {
            if self.lines.pop().is_some() {
                *total = total.saturating_sub(1);
            }
            self.partial_lines -= 1;
        }

        for segment in complete.split_inclusive('\n') {
            let raw = segment.trim_end_matches('\n');
            for wrapped in wrap_plain(raw, width) {
                self.lines.push(body_line(theme, wrapped));
                *total += 1;
            }
        }

        if !partial.is_empty() {
            self.pending = partial.to_string();
            let before = self.lines.len();
            for wrapped in wrap_plain(partial, width) {
                self.lines.push(body_line(theme, wrapped));
                *total += 1;
            }
            self.partial_lines = self.lines.len() - before;
        }
    }
}

fn body_line(theme: &Theme, text: String) -> Line<'static> {
    Line::from(text).style(Style::default().bg(theme.bg).fg(theme.fg))
}

/// Wrap a single raw text line at `width`, preserving leading indentation and
/// keeping blank lines intact.
fn wrap_plain(text: &str, width: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let indent = text.chars().take_while(|&c| c == ' ').count();
    let ws: String = text.chars().take(indent).collect();
    let options = textwrap::Options::new(width)
        .wrap_algorithm(textwrap::WrapAlgorithm::FirstFit)
        .initial_indent(&ws);
    let rest = &text[ws.len()..];
    let wrapped = textwrap::wrap(rest, options);
    if wrapped.is_empty() {
        vec![text.to_string()]
    } else {
        wrapped.into_iter().map(|c| c.into_owned()).collect()
    }
}

/// Convert a markdown-rendered (ANSI) line into a styled ratatui `Line`.
fn ansi_to_line(text: &str, theme: &Theme) -> Line<'static> {
    let base = Style::default().bg(theme.bg).fg(theme.fg);
    let mut style = base;
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            if !current.is_empty() {
                spans.push(Span::styled(std::mem::take(&mut current), style));
            }
            chars.next(); // consume '['
            let mut params = String::new();
            while let Some(&n) = chars.peek() {
                chars.next();
                if n == 'm' {
                    break;
                }
                params.push(n);
            }
            style = apply_sgr(base, style, &params);
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        spans.push(Span::styled(current, style));
    }
    if spans.is_empty() {
        spans.push(Span::styled(String::new(), base));
    }
    Line::from(spans).style(base)
}

/// Apply a single SGR escape parameter list to the current style.
fn apply_sgr(base: Style, mut style: Style, params: &str) -> Style {
    let parts: Vec<&str> = params.split(';').collect();
    let mut i = 0;
    while i < parts.len() {
        match parts[i] {
            "" | "0" => style = base,
            "1" => style = style.add_modifier(Modifier::BOLD),
            "4" => style = style.add_modifier(Modifier::UNDERLINED),
            "22" => style = style.remove_modifier(Modifier::BOLD),
            "24" => style = style.remove_modifier(Modifier::UNDERLINED),
            "39" => style = style.fg(Color::Reset),
            "49" => style = style.bg(Color::Reset),
            "38" | "48" => {
                let is_fg = parts[i] == "38";
                i += 1;
                if i < parts.len() {
                    match parts[i] {
                        "5" => {
                            i += 1;
                            if i < parts.len() {
                                if let Ok(n) = parts[i].parse::<u8>() {
                                    let color = Color::Indexed(n);
                                    style = if is_fg {
                                        style.fg(color)
                                    } else {
                                        style.bg(color)
                                    };
                                }
                            }
                        }
                        "2" => {
                            i += 1;
                            if i + 2 < parts.len() {
                                let r = parts[i].parse().unwrap_or(0);
                                let g = parts[i + 1].parse().unwrap_or(0);
                                let b = parts[i + 2].parse().unwrap_or(0);
                                let color = Color::Rgb(r, g, b);
                                style = if is_fg {
                                    style.fg(color)
                                } else {
                                    style.bg(color)
                                };
                                i += 2;
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        if i < parts.len() {
            if let Ok(code) = parts[i].parse::<u8>() {
                match code {
                    30..=37 | 90..=97 => {
                        style = style.fg(basic_color(code));
                    }
                    40..=47 | 100..=107 => {
                        style = style.bg(basic_color(code - 10));
                    }
                    _ => {}
                }
            }
        }
        i += 1;
    }
    style
}

fn basic_color(n: u8) -> Color {
    match n {
        30 => Color::Black,
        31 => Color::Red,
        32 => Color::Green,
        33 => Color::Yellow,
        34 => Color::Blue,
        35 => Color::Magenta,
        36 => Color::Cyan,
        37 => Color::White,
        90 => Color::Gray,
        91 => Color::LightRed,
        92 => Color::LightGreen,
        93 => Color::LightYellow,
        94 => Color::LightBlue,
        95 => Color::LightMagenta,
        96 => Color::LightCyan,
        _ => Color::White,
    }
}

fn syntect_theme(mode: ThemeMode) -> Option<syntect::highlighting::Theme> {
    crate::config::builtin_syntax_theme(mode == ThemeMode::Light).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme() -> Theme {
        Theme::dark()
    }

    #[test]
    fn streaming_appends_incrementally() {
        let mut sb = Scrollback::new(ThemeMode::Dark, false);
        let theme = theme();
        sb.append_stream_delta(&theme, "hello ");
        sb.prepare(&theme, 40);
        let before = sb.line_count();
        assert_eq!(before, 2); // header + partial line

        // Completing the partial line re-wraps it in place (no growth)...
        sb.append_stream_delta(&theme, "world");
        sb.prepare(&theme, 40);
        assert_eq!(sb.line_count(), 2);

        // ...but a newline adds a new body line.
        sb.append_stream_delta(&theme, "\nsecond");
        sb.prepare(&theme, 40);
        let after = sb.line_count();
        assert_eq!(after, 3, "newline should grow the line count");

        // exactly one entry, still streaming
        assert_eq!(sb.entries.len(), 1);
        assert!(sb.entries[0].streaming);
        assert_eq!(sb.entries[0].text, "hello world\nsecond");
    }

    #[test]
    fn no_full_rebuild_on_stream_tokens() {
        let mut sb = Scrollback::new(ThemeMode::Dark, false);
        let theme = theme();
        sb.push_user(&theme, "first user message".into());
        sb.prepare(&theme, 40);
        sb.append_stream_delta(&theme, "token one ");
        sb.prepare(&theme, 40);

        let lines_before: Vec<String> = sb
            .visible_lines(0, usize::MAX)
            .into_iter()
            .map(|l| l.to_string())
            .collect();

        // stream more tokens
        for tok in ["token two ", "token three ", "token four "] {
            sb.append_stream_delta(&theme, tok);
            sb.prepare(&theme, 40);
        }
        let lines_after: Vec<String> = sb
            .visible_lines(0, usize::MAX)
            .into_iter()
            .map(|l| l.to_string())
            .collect();

        // The user message + header lines must be byte-identical (reused),
        // only the streamed tail changed.
        assert!(lines_before.len() >= 2);
        assert_eq!(&lines_before[..2], &lines_after[..2]);
    }

    #[test]
    fn finish_stream_finalizes_and_keeps_text() {
        let mut sb = Scrollback::new(ThemeMode::Dark, false);
        let theme = theme();
        sb.append_stream_delta(&theme, "partial ");
        sb.prepare(&theme, 40);
        sb.append_stream_delta(&theme, "response");
        sb.finish_stream(&theme, "final response".into());
        sb.prepare(&theme, 40);
        assert_eq!(sb.line_count(), 2); // header + body
        assert!(!sb.entries[0].streaming);
        assert_eq!(sb.entries[0].text, "final response");
    }

    #[test]
    fn interrupt_preserves_partial() {
        let mut sb = Scrollback::new(ThemeMode::Dark, false);
        let theme = theme();
        sb.append_stream_delta(&theme, "this was cut off mid-");
        sb.prepare(&theme, 40);
        sb.finalize_interrupt(&theme);
        sb.prepare(&theme, 40);
        assert_eq!(sb.line_count(), 2); // header + partial line
        assert!(!sb.entries[0].streaming);
        assert_eq!(sb.entries[0].text, "this was cut off mid-");
    }

    #[test]
    fn multiline_delta_wraps_complete_lines_once() {
        let mut sb = Scrollback::new(ThemeMode::Dark, false);
        let theme = theme();
        sb.append_stream_delta(&theme, "aaaa\nbbbb\ncccc");
        sb.prepare(&theme, 80);
        // header + 3 body lines
        assert_eq!(sb.line_count(), 4);

        // Append after a completed line: only the tail is rewrapped.
        sb.append_stream_delta(&theme, "\ndddd");
        sb.prepare(&theme, 80);
        assert_eq!(sb.line_count(), 5);
    }

    #[test]
    fn resize_rebuilds_at_new_width() {
        let mut sb = Scrollback::new(ThemeMode::Dark, false);
        let theme = theme();
        sb.push_user(&theme, "hello world this is a long line that will wrap".into());
        sb.prepare(&theme, 10);
        let narrow = sb.line_count();
        sb.prepare(&theme, 60);
        let wide = sb.line_count();
        assert!(narrow > wide);
    }

    #[test]
    fn wrap_plain_keeps_blank_lines() {
        assert_eq!(wrap_plain("", 10), vec![String::new()]);
        assert_eq!(wrap_plain("one two", 10), vec!["one two".to_string()]);
        let wrapped = wrap_plain("one two three", 7);
        assert_eq!(wrapped, vec!["one two".to_string(), "three".to_string()]);
    }

    #[test]
    fn ansi_to_line_parses_styles() {
        let theme = theme();
        let line = ansi_to_line("\u{1b}[1m\u{1b}[38;5;208mbold orange\u{1b}[0m rest", &theme);
        assert_eq!(line.spans.len(), 2);
        assert!(line.spans[0]
            .style
            .add_modifier
            .contains(Modifier::BOLD));
        assert_eq!(
            line.spans[0].style.fg,
            Some(ratatui::style::Color::Indexed(208))
        );
        assert_eq!(line.spans[0].content, "bold orange");
        assert_eq!(line.spans[1].content, " rest");
    }

    #[test]
    fn same_as_detects_matching_messages() {
        let mut sb = Scrollback::new(ThemeMode::Dark, false);
        let theme = theme();
        sb.push_user(&theme, "hello".into());
        sb.append_stream_delta(&theme, "response");
        sb.finish_stream(&theme, "response".into());

        let views = vec![
            MessageView {
                role: MessageRole::User,
                text: "hello".into(),
                streaming: false,
                tool_calls: vec![],
            },
            MessageView {
                role: MessageRole::Assistant,
                text: "response".into(),
                streaming: false,
                tool_calls: vec![],
            },
        ];
        assert!(sb.same_as(&views));

        // A differing text means the log needs a rebuild.
        let changed = vec![
            views[0].clone(),
            MessageView {
                role: MessageRole::Assistant,
                text: "different".into(),
                streaming: false,
                tool_calls: vec![],
            },
        ];
        assert!(!sb.same_as(&changed));

        // A different length also means rebuild.
        assert!(!sb.same_as(&views[..1]));
    }
}
