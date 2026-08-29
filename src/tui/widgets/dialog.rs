use crate::tui::state::AppState;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
    Frame,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogKind {
    Palette,
    SessionList,
    ModelPicker,
    RolePicker,
    Help,
}

#[derive(Debug, Clone)]
pub struct PaletteCommand {
    pub id: String,
    pub title: String,
    pub category: String,
}

impl PaletteCommand {
    pub fn defaults() -> Vec<Self> {
        vec![
            Self {
                id: "session.list".into(),
                title: "Switch session".into(),
                category: "Session".into(),
            },
            Self {
                id: "session.new".into(),
                title: "New session".into(),
                category: "Session".into(),
            },
            Self {
                id: "model.list".into(),
                title: "Switch model".into(),
                category: "Model".into(),
            },
            Self {
                id: "role.list".into(),
                title: "Switch role".into(),
                category: "Role".into(),
            },
            Self {
                id: "view.sidebar".into(),
                title: "Toggle sidebar".into(),
                category: "View".into(),
            },
            Self {
                id: "help".into(),
                title: "Show help".into(),
                category: "Help".into(),
            },
            Self {
                id: "quit".into(),
                title: "Quit".into(),
                category: "App".into(),
            },
        ]
    }
}

#[derive(Debug, Clone, Default)]
pub struct DialogStack(Vec<DialogKind>);

impl DialogStack {
    pub fn push(&mut self, dialog: DialogKind) {
        self.0.push(dialog);
    }

    pub fn pop(&mut self) -> Option<DialogKind> {
        self.0.pop()
    }

    pub fn top(&self) -> Option<&DialogKind> {
        self.0.last()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

/// A single selectable row in a dialog. `header` rows render as violet section
/// titles; `selected` rows render with a full-row primary (peach) highlight.
struct DialogRow {
    label: String,
    hint: Option<&'static str>,
    header: bool,
    selected: bool,
}

/// Renders an opencode-style flat modal: no box borders, panel background,
/// bold title + dim `esc`, optional search field, violet section headers,
/// full-row peach selection highlight, right-aligned keybinding hints, and a
/// dimmed scrim behind the panel.
pub fn render_dialog(f: &mut Frame, area: Rect, state: &AppState, dialog: &DialogKind) {
    let (title, rows, search) = dialog_rows(state, dialog);

    // --- Compute panel geometry before building padded lines ---
    let max_label = rows.iter().map(|r| r.label.chars().count()).max().unwrap_or(0);
    let max_hint = rows.iter().filter_map(|r| r.hint).map(|h| h.chars().count()).max().unwrap_or(0);
    let pad = 4;
    let needed = max_label + if max_hint > 0 { 3 + max_hint } else { 0 } + pad;
    let width = needed
        .min(area.width as usize)
        .max(1) as u16;
    let inner_w = width.saturating_sub(4) as usize;

    // --- Scrim (dim the whole screen behind the panel) ---
    let scrim = Paragraph::new("")
        .style(Style::default().bg(state.theme.panel_bg));
    f.render_widget(Clear, area);
    f.render_widget(scrim, area);

    // --- Build the body lines (header + search + rows) ---
    let mut body: Vec<Line> = Vec::new();
    let title_w = title.chars().count();
    body.push(Line::from(vec![
        Span::styled(&title, Style::default().fg(state.theme.fg).add_modifier(Modifier::BOLD)),
        Span::raw(" ".repeat(inner_w.saturating_sub(title_w).max(1))),
        Span::styled("esc", Style::default().fg(state.theme.muted)),
    ]));

    if let Some(q) = search {
        body.push(Line::from(vec![
            Span::styled(
                ">",
                Style::default().fg(state.theme.primary),
            ),
            Span::raw(format!(" {}", q)),
        ]));
    }

    for row in &rows {
        let content_w = if row.hint.is_some() {
            inner_w.saturating_sub(3 + max_hint)
        } else {
            inner_w
        };
        let label = truncate_to(&row.label, content_w);
        let pad_spaces = content_w.saturating_sub(label.chars().count());
        let mut spans: Vec<Span> = Vec::new();
        let (label_style, row_bg) = if row.header {
            (Style::default().fg(state.theme.accent).add_modifier(Modifier::BOLD), None)
        } else if row.selected {
            (Style::default().fg(state.theme.bg), Some(state.theme.primary))
        } else {
            (Style::default().fg(state.theme.fg), None)
        };
        spans.push(Span::styled(label, label_style.bg(row_bg.unwrap_or(state.theme.panel_bg))));
        if let Some(hint) = row.hint {
            spans.push(Span::styled(
                format!("{}{}", " ".repeat(pad_spaces + 1), hint),
                Style::default().fg(state.theme.muted).bg(row_bg.unwrap_or(state.theme.panel_bg)),
            ));
        }
        body.push(Line::from(spans));
    }

    // Actions row at the bottom (opencode-style dim key hints).
    if let Some(actions) = dialog_actions(dialog) {
        body.push(Line::from(Span::styled(
            actions,
            Style::default().fg(state.theme.muted),
        )));
    }

    let height = (body.len() as u16 + 2).min(area.height.saturating_sub(2)).max(4);
    let x = area.width.saturating_sub(width) / 2;
    // Anchor ~25% from the top (opencode style), not center.
    let y = (area.height.saturating_sub(height) / 4).max(1);
    let panel = Rect::new(x, y, width, height);

    let widget = Paragraph::new(body)
        .style(Style::default().bg(state.theme.panel_bg).fg(state.theme.fg));
    f.render_widget(widget, panel);
}

fn dialog_rows(state: &AppState, dialog: &DialogKind) -> (String, Vec<DialogRow>, Option<String>) {
    match dialog {
        DialogKind::Palette => {
            let commands = state.filtered_palette_commands();
            let mut rows: Vec<DialogRow> = Vec::new();
            let mut last_cat: Option<String> = None;
            for (item_idx, cmd) in commands.iter().enumerate() {
                if last_cat.as_deref() != Some(cmd.category.as_str()) {
                    rows.push(DialogRow {
                        label: cmd.category.clone(),
                        hint: None,
                        header: true,
                        selected: false,
                    });
                    last_cat = Some(cmd.category.clone());
                }
                rows.push(DialogRow {
                    label: cmd.title.clone(),
                    hint: palette_hint(&cmd.id),
                    header: false,
                    selected: item_idx == state.palette_selected,
                });
            }
            (String::from("Commands"), rows, Some(state.palette_filter.clone()))
        }
        DialogKind::SessionList => {
            let items = session_rows(state);
            (String::from("Sessions"), items, None)
        }
        DialogKind::ModelPicker => {
            let items = picker_rows(state, "models");
            (String::from("Models"), items, None)
        }
        DialogKind::RolePicker => {
            let items = picker_rows(state, "roles");
            (String::from("Roles"), items, None)
        }
        DialogKind::Help => (
            String::from("Help"),
            vec![
                DialogRow { label: "Enter: submit  Shift+Enter: newline  Esc: abort/close".into(), hint: None, header: false, selected: false },
                DialogRow { label: "Ctrl+P: palette  Ctrl+B: sidebar  Ctrl+N: home".into(), hint: None, header: false, selected: false },
                DialogRow { label: "Ctrl+S: sessions  Ctrl+M: models  Ctrl+R: roles".into(), hint: None, header: false, selected: false },
                DialogRow { label: ".help .model .role .session .exit".into(), hint: None, header: false, selected: false },
            ],
            None,
        ),
    }
}

/// Items for model/role pickers come from `state.sidebar_lines`.
fn picker_rows(state: &AppState, _kind: &str) -> Vec<DialogRow> {
    state
        .sidebar_lines
        .iter()
        .enumerate()
        .map(|(i, s)| DialogRow {
            label: s.clone(),
            hint: None,
            header: false,
            selected: i == state.palette_selected,
        })
        .collect()
}

/// Sessions grouped by recency (Today / Yesterday / Earlier), opencode style.
fn session_rows(state: &AppState) -> Vec<DialogRow> {
    let mut rows: Vec<DialogRow> = Vec::new();
    let mut last_bucket: Option<&'static str> = None;
    for (i, name) in state.sidebar_lines.iter().enumerate() {
        let bucket = date_bucket(state.session_times.get(i).copied().flatten());
        if Some(bucket) != last_bucket {
            rows.push(DialogRow {
                label: bucket.to_string(),
                hint: None,
                header: true,
                selected: false,
            });
            last_bucket = Some(bucket);
        }
        rows.push(DialogRow {
            label: name.clone(),
            hint: None,
            header: false,
            selected: i == state.palette_selected,
        });
    }
    rows
}

fn date_bucket(updated: Option<std::time::SystemTime>) -> &'static str {
    let Some(updated) = updated else {
        return "Earlier";
    };
    let Ok(age) = updated.elapsed() else {
        return "Earlier";
    };
    let day = std::time::Duration::from_secs(24 * 60 * 60);
    if age < day {
        "Today"
    } else if age < 2 * day {
        "Yesterday"
    } else {
        "Earlier"
    }
}

fn palette_hint(id: &str) -> Option<&'static str> {
    match id {
        "session.list" => Some("ctrl+s"),
        "session.new" => Some("ctrl+n"),
        "model.list" => Some("ctrl+m"),
        "role.list" => Some("ctrl+r"),
        "view.sidebar" => Some("ctrl+b"),
        "help" => Some("ctrl+h"),
        "quit" => Some("esc esc"),
        _ => None,
    }
}

/// Bottom action-hint row per dialog kind.
fn dialog_actions(dialog: &DialogKind) -> Option<&'static str> {
    match dialog {
        DialogKind::Palette => None,
        DialogKind::SessionList => Some("enter switch   ctrl+d delete   esc close"),
        DialogKind::ModelPicker => Some("enter switch   esc close"),
        DialogKind::RolePicker => Some("enter switch   esc close"),
        DialogKind::Help => None,
    }
}

fn truncate_to(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        text.to_string()
    } else {
        let mut out: String = text.chars().take(max.saturating_sub(3)).collect();
        out.push_str("...");
        out
    }
}

#[cfg(test)]
mod stack_tests {
    use super::*;

    #[test]
    fn push_pop_top() {
        let mut stack = DialogStack::default();
        assert!(stack.is_empty());
        stack.push(DialogKind::Help);
        stack.push(DialogKind::ModelPicker);
        assert_eq!(stack.len(), 2);
        assert!(matches!(stack.top(), Some(DialogKind::ModelPicker)));
        let popped = stack.pop().unwrap();
        assert!(matches!(popped, DialogKind::ModelPicker));
        assert!(matches!(stack.top(), Some(DialogKind::Help)));
        let _ = stack.pop().unwrap();
        assert!(stack.is_empty());
    }

    #[test]
    fn esc_pops_top_first() {
        let mut stack = DialogStack::default();
        stack.push(DialogKind::Help);
        stack.push(DialogKind::Palette);
        // Top dialog handles Esc first
        if matches!(stack.top(), Some(DialogKind::Palette)) {
            let _ = stack.pop();
        }
        assert!(matches!(stack.top(), Some(DialogKind::Help)));
    }
}
