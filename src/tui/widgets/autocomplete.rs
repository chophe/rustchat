use crate::tui::state::AppState;
use crate::tui::widgets::PaletteCommand;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use tui_textarea::TextArea;

const MAX_FILE_INDEX: usize = 200;
const MAX_CANDIDATES: usize = 8;

/// Floating autocomplete surface driven by the current input word.
///
/// Candidates update reactively as the word changes; the popup is "open" as
/// soon as there are matches. It is only *activated* (Tab) before Enter is
/// allowed to accept a candidate, so Enter-to-submit is never hijacked.
pub struct AutocompleteState {
    matcher: SkimMatcherV2,
    file_index: Vec<String>,
    file_index_built: bool,
    candidates: Vec<String>,
    selected: usize,
    prefix: String,
    active: bool,
}

impl Default for AutocompleteState {
    fn default() -> Self {
        Self {
            matcher: SkimMatcherV2::default().smart_case(),
            file_index: vec![],
            file_index_built: false,
            candidates: vec![],
            selected: 0,
            prefix: String::new(),
            active: false,
        }
    }
}

impl AutocompleteState {
    pub fn is_open(&self) -> bool {
        !self.candidates.is_empty()
    }

    pub fn active(&self) -> bool {
        self.active && self.is_open()
    }

    pub fn close(&mut self) {
        self.candidates.clear();
        self.selected = 0;
        self.active = false;
        self.prefix.clear();
    }

    /// Recompute candidates for the current word. Called after the textarea
    /// receives a printable key.
    pub fn on_input(&mut self, textarea: &TextArea<'static>) {
        let word = current_word(textarea);
        if word == self.prefix {
            return;
        }
        self.prefix = word.to_string();
        self.selected = 0;
        self.active = false;
        if word.is_empty() {
            self.candidates.clear();
            return;
        }
        self.candidates = self.find_matches(&word);
    }

    /// Tab: activate the popup and move to the next candidate.
    /// Returns true if the key was consumed (there are candidates).
    pub fn cycle_next(&mut self) -> bool {
        if self.candidates.is_empty() {
            return false;
        }
        self.active = true;
        self.selected = (self.selected + 1) % self.candidates.len();
        true
    }

    /// Shift+Tab: activate the popup and move to the previous candidate.
    pub fn cycle_prev(&mut self) -> bool {
        if self.candidates.is_empty() {
            return false;
        }
        self.active = true;
        self.selected = self
            .selected
            .checked_sub(1)
            .unwrap_or(self.candidates.len() - 1);
        true
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn candidates(&self) -> &[String] {
        &self.candidates
    }

    /// Replace the current word in the textarea with the selected candidate.
    pub fn accept(&mut self, textarea: &mut TextArea<'static>) {
        let Some(candidate) = self.candidates.get(self.selected).cloned() else {
            self.close();
            return;
        };
        let (row, col) = textarea.cursor();
        let mut lines: Vec<String> = textarea.lines().to_vec();
        if let Some(line) = lines.get_mut(row) {
            let prefix: String = line.chars().take(col).collect();
            let word_start = prefix
                .char_indices()
                .rfind(|(_, c)| c.is_whitespace())
                .map(|(i, c)| i + c.len_utf8())
                .unwrap_or(0);
            let mut new_line = String::new();
            new_line.push_str(&prefix[..word_start]);
            new_line.push_str(&candidate);
            *line = new_line;
        }
        *textarea = TextArea::from(lines);
        textarea.set_placeholder_text(
            "Type a message... (Enter submit, Shift+Enter newline, Esc abort, Ctrl+P palette)",
        );
        self.close();
    }

    fn find_matches(&mut self, word: &str) -> Vec<String> {
        if let Some(rest) = word.strip_prefix('/') {
            return PaletteCommand::defaults()
                .into_iter()
                .filter_map(|cmd| {
                    let hay = format!("{} {}", cmd.id, cmd.title);
                    self.matcher
                        .fuzzy_match(&hay, rest)
                        .map(|score| (score, format!("/{}", cmd.id)))
                })
                .map(|(_, cand)| cand)
                .collect();
        }
        if !self.file_index_built {
            self.file_index = walk_files();
            self.file_index_built = true;
        }
        let mut scored: Vec<(i64, String)> = self
            .file_index
            .iter()
            .filter_map(|path| {
                self.matcher
                    .fuzzy_match(path, word)
                    .map(|score| (score, path.clone()))
            })
            .collect();
        scored.sort_unstable_by_key(|(score, _)| std::cmp::Reverse(*score));
        scored
            .into_iter()
            .take(MAX_CANDIDATES)
            .map(|(_, path)| path)
            .collect()
    }
}

fn current_word(textarea: &TextArea<'static>) -> String {
    let (row, col) = textarea.cursor();
    let Some(line) = textarea.lines().get(row) else {
        return String::new();
    };
    let prefix: String = line.chars().take(col).collect();
    match prefix.rfind(|c: char| c.is_whitespace()) {
        Some(i) => prefix[i + 1..].to_string(),
        None => prefix,
    }
}

/// Shallow, bounded directory walk used to build the file candidate index.
fn walk_files() -> Vec<String> {
    let mut out = Vec::new();
    let Ok(cwd) = std::env::current_dir() else {
        return out;
    };
    let Ok(entries) = std::fs::read_dir(&cwd) else {
        return out;
    };
    for entry in entries.flatten() {
        if out.len() >= MAX_FILE_INDEX {
            break;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let display = format!("./{}", name);
        if path.is_dir() {
            out.push(format!("{display}/"));
            if let Ok(sub) = std::fs::read_dir(&path) {
                for sub_entry in sub.flatten() {
                    if out.len() >= MAX_FILE_INDEX {
                        break;
                    }
                    let sub_name = sub_entry.file_name().to_string_lossy().to_string();
                    if sub_name.starts_with('.') {
                        continue;
                    }
                    let sub_display = format!("{display}/{sub_name}");
                    if sub_entry.path().is_dir() {
                        out.push(format!("{sub_display}/"));
                    } else {
                        out.push(sub_display);
                    }
                }
            }
        } else {
            out.push(display);
        }
    }
    out
}

/// Draw the candidate list as a floating panel just above the footer.
pub fn render_autocomplete(f: &mut Frame, area: Rect, state: &AppState) {
    let candidates = state.autocomplete.candidates();
    if candidates.is_empty() {
        return;
    }
    let width = area.width.saturating_sub(4).clamp(20, 60).min(area.width);
    let height = (candidates.len() as u16)
        .min(MAX_CANDIDATES as u16)
        .min(area.height.saturating_sub(1)) + 1;
    let x = area.width.saturating_sub(width) / 2;
    let y = area.y.saturating_sub(height);
    let popup = Rect::new(x, y, width, height);

    let mut lines: Vec<Line> = Vec::new();
    for (i, candidate) in candidates.iter().enumerate().take(MAX_CANDIDATES) {
        let selected = i == state.autocomplete.selected();
        let marker = if selected { "> " } else { "  " };
        let text = truncate(candidate, width as usize);
        lines.push(Line::from(vec![
            Span::raw(marker),
            Span::styled(
                text,
                if selected {
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(state.theme.accent)
                } else {
                    Style::default().fg(state.theme.fg)
                },
            ),
        ]));
    }

    f.render_widget(Clear, popup);
    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Complete ")
                .border_style(state.theme.dialog_border),
        )
        .style(Style::default().bg(state.theme.bg).fg(state.theme.fg));
    f.render_widget(widget, popup);
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let mut out: String = text.chars().take(max.saturating_sub(3)).collect();
        out.push_str("...");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn textarea_from(lines: &[&str]) -> TextArea<'static> {
        TextArea::from(lines.to_vec())
    }

    #[test]
    fn current_word_from_cursor() {
        let mut ta = textarea_from(&["hello world"]);
        ta.move_cursor(tui_textarea::CursorMove::Jump(0, 11));
        assert_eq!(current_word(&ta), "world");
        // Cursor right after the space: no word yet.
        ta.move_cursor(tui_textarea::CursorMove::Jump(0, 6));
        assert_eq!(current_word(&ta), "");
    }

    #[test]
    fn command_matches_slash_prefix() {
        let mut ac = AutocompleteState::default();
        let mut ta = textarea_from(&["/mod"]);
        ta.move_cursor(tui_textarea::CursorMove::Jump(0, 4));
        ac.on_input(&ta);
        assert!(ac.is_open());
        assert!(ac.candidates().iter().any(|c| c == "/model.list"));
    }

    #[test]
    fn tab_activates_and_cycles() {
        let mut ac = AutocompleteState::default();
        let mut ta = textarea_from(&["/s"]);
        ta.move_cursor(tui_textarea::CursorMove::Jump(0, 2));
        ac.on_input(&ta);
        let n = ac.candidates().len();
        assert!(n >= 2, "expected multiple command matches, got {n}");
        assert!(!ac.active());
        assert!(ac.cycle_next());
        assert!(ac.active());
        assert_eq!(ac.selected(), 1 % n);
        assert!(ac.cycle_prev());
        assert_eq!(ac.selected(), 0);
    }

    #[test]
    fn enter_only_accepts_when_active() {
        let mut ac = AutocompleteState::default();
        let mut ta = textarea_from(&["/mod"]);
        ta.move_cursor(tui_textarea::CursorMove::Jump(0, 4));
        ac.on_input(&ta);
        // Typing only opens the popup; it is not activated until Tab.
        assert!(ac.is_open());
        assert!(!ac.active());
        let _ = ac.cycle_next();
        assert!(ac.active());
    }

    #[test]
    fn accept_replaces_word() {
        let mut ac = AutocompleteState::default();
        let mut ta = textarea_from(&["/mod"]);
        ta.move_cursor(tui_textarea::CursorMove::Jump(0, 4));
        ac.on_input(&ta);
        let _ = ac.cycle_next();
        let mut target = textarea_from(&["/mod"]);
        target.move_cursor(tui_textarea::CursorMove::Jump(0, 4));
        ac.accept(&mut target);
        assert!(!ac.is_open());
        assert_eq!(target.lines().join("\n"), "/model.list");
    }

    #[test]
    fn no_candidates_means_not_open() {
        // Force an empty file index so the test does not depend on the cwd.
        let mut ac = AutocompleteState {
            file_index_built: true,
            ..Default::default()
        };
        let mut ta = textarea_from(&["hello there"]);
        ta.move_cursor(tui_textarea::CursorMove::Jump(0, 11));
        ac.on_input(&ta);
        // "there" fuzzy matches nothing in an empty index → closed
        assert!(!ac.is_open());
    }
}
