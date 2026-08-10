use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub mode: ThemeMode,
    pub bg: Color,
    pub fg: Color,
    pub header_bg: Color,
    pub header_fg: Color,
    pub status_bg: Color,
    pub status_fg: Color,
    pub user_label: Style,
    pub assistant_label: Style,
    pub system_label: Style,
    pub prompt_border: Style,
    pub sidebar_border: Style,
    pub dialog_border: Style,
    pub accent: Color,
    pub dim: Color,
    pub error: Color,
    pub toast: Style,
}

impl Theme {
    pub fn detect() -> Self {
        match std::env::var("COLORFGBG")
            .ok()
            .and_then(|v| v.split(';').nth(1)?.parse::<u8>().ok())
        {
            Some(bg) if bg <= 6 => Self::dark(),
            Some(_) => Self::light(),
            None => Self::dark(),
        }
    }

    pub fn dark() -> Self {
        Self {
            mode: ThemeMode::Dark,
            bg: Color::Rgb(30, 30, 30),
            fg: Color::Rgb(220, 220, 220),
            header_bg: Color::Rgb(40, 40, 40),
            header_fg: Color::Rgb(180, 180, 255),
            status_bg: Color::Rgb(35, 35, 35),
            status_fg: Color::Rgb(160, 160, 160),
            user_label: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            assistant_label: Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            system_label: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            prompt_border: Style::default().fg(Color::Rgb(100, 100, 140)),
            sidebar_border: Style::default().fg(Color::Rgb(80, 80, 100)),
            dialog_border: Style::default().fg(Color::Rgb(120, 120, 180)),
            accent: Color::Rgb(130, 170, 255),
            dim: Color::Rgb(120, 120, 120),
            error: Color::Red,
            toast: Style::default().fg(Color::Rgb(200, 200, 255)),
        }
    }

    pub fn light() -> Self {
        Self {
            mode: ThemeMode::Light,
            bg: Color::Rgb(250, 250, 250),
            fg: Color::Rgb(30, 30, 30),
            header_bg: Color::Rgb(240, 240, 245),
            header_fg: Color::Rgb(40, 40, 120),
            status_bg: Color::Rgb(245, 245, 245),
            status_fg: Color::Rgb(80, 80, 80),
            user_label: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            assistant_label: Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            system_label: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            prompt_border: Style::default().fg(Color::Rgb(120, 120, 160)),
            sidebar_border: Style::default().fg(Color::Rgb(100, 100, 120)),
            dialog_border: Style::default().fg(Color::Rgb(100, 100, 160)),
            accent: Color::Rgb(50, 80, 200),
            dim: Color::Rgb(130, 130, 130),
            error: Color::Red,
            toast: Style::default().fg(Color::Rgb(40, 40, 100)),
        }
    }
}
