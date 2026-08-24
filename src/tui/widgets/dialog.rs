use crate::tui::state::AppState;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

#[derive(Debug, Clone)]
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

pub fn render_dialog(f: &mut Frame, area: Rect, state: &AppState, dialog: &DialogKind) {
    let (title, lines) = match dialog {
        DialogKind::Palette => {
            let mut lines = vec![Line::from(Span::styled(
                "Type to filter, Enter to select, Esc to close",
                Style::default().fg(state.theme.dim),
            ))];
            for (i, cmd) in state.filtered_palette_commands().iter().enumerate() {
                let prefix = if i == state.palette_selected {
                    "> "
                } else {
                    "  "
                };
                lines.push(Line::from(vec![
                    Span::raw(prefix),
                    Span::styled(
                        format!("{} ", cmd.title),
                        if i == state.palette_selected {
                            Style::default().add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        },
                    ),
                    Span::styled(
                        format!("({})", cmd.category),
                        Style::default().fg(state.theme.dim),
                    ),
                ]));
            }
            (" Command Palette ", lines)
        }
        DialogKind::SessionList => (
            " Sessions ",
            state
                .sidebar_lines
                .iter()
                .map(|s| Line::from(s.as_str()))
                .collect(),
        ),
        DialogKind::ModelPicker => (
            " Models ",
            vec![Line::from("Use .model <name> or select from list")],
        ),
        DialogKind::RolePicker => (
            " Roles ",
            vec![Line::from("Use .role <name> or select from list")],
        ),
        DialogKind::Help => (
            " Help ",
            vec![
                Line::from("Enter: submit  Shift+Enter: newline  Esc: abort/close"),
                Line::from("Ctrl+P: palette  Ctrl+B: sidebar  Ctrl+N: home"),
                Line::from("Ctrl+S: sessions  Ctrl+M: models  Ctrl+R: roles"),
                Line::from(".help .model .role .session .exit"),
            ],
        ),
        };

    let height = (lines.len() as u16 + 4).min(area.height.saturating_sub(2)).max(5);
    let width = area.width.saturating_sub(4).max(20).min(area.width);
    let x = area.width.saturating_sub(width) / 2;
    let y = area.height.saturating_sub(height) / 2;
    let dialog_area = Rect::new(x, y, width, height);

    f.render_widget(Clear, dialog_area);
    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(state.theme.dialog_border),
        )
        .wrap(Wrap { trim: false })
        .style(Style::default().bg(state.theme.bg).fg(state.theme.fg));
    f.render_widget(widget, dialog_area);
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
