use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

/// The opencode-style palette (see DESIGN.md). Dark voice: near-black neutral
/// ramp with a single warm Ember Peach primary, Structure Violet structure,
/// and Signal Blue functional accent. Light mode inverts the voice.
#[derive(Debug, Clone)]
pub struct Theme {
    pub mode: ThemeMode,
    /// Screen background (reading surface). Never pure black.
    pub bg: Color,
    /// Raised panel background (dialogs, prompt, pickers).
    pub panel_bg: Color,
    /// Primary text.
    pub fg: Color,
    /// Muted / secondary text.
    pub muted: Color,
    /// Resting border color.
    #[allow(dead_code)] // reserved for Phase 5
    pub border: Color,
    /// Active / hover border color.
    #[allow(dead_code)] // reserved for Phase 5 (active surfaces)
    pub border_active: Color,
    /// Warm brand primary (selection highlight, spinner).
    pub primary: Color,
    /// Signal Blue (links, user labels, focus accents).
    #[allow(dead_code)] // reserved for Phase 5
    pub secondary: Color,
    /// Structure Violet (section headers, structure).
    pub accent: Color,
    /// Functional status colors (reserved for Phase 5: toasts/errors).
    #[allow(dead_code)]
    pub error: Color,
    #[allow(dead_code)]
    pub warning: Color,
    #[allow(dead_code)]
    pub success: Color,
    /// Status/footer bar surface.
    pub status_bg: Color,
    pub status_fg: Color,
    /// Transcript role labels.
    pub user_label: Style,
    pub assistant_label: Style,
    pub system_label: Style,
    /// Border styles for surfaces (all default to the same neutral ramp).
    pub prompt_border: Style,
    pub sidebar_border: Style,
    pub dialog_border: Style,
    /// Muted text (alias of `muted` for legacy call sites).
    pub dim: Color,
    pub toast: Style,
}

impl Theme {
    pub fn detect() -> Self {
        // COLORFGBG: "foreground;background" index; bg 0-6 implies a dark terminal.
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
        let primary = Color::Rgb(0xfa, 0xb2, 0x83);
        let secondary = Color::Rgb(0x5c, 0x9c, 0xf5);
        let accent = Color::Rgb(0x9d, 0x7c, 0xd8);
        let border = Color::Rgb(0x48, 0x48, 0x48);
        let muted = Color::Rgb(0x80, 0x80, 0x80);
        let border_style = Style::default().fg(border);
        Self {
            mode: ThemeMode::Dark,
            bg: Color::Rgb(0x0a, 0x0a, 0x0a),
            panel_bg: Color::Rgb(0x14, 0x14, 0x14),
            fg: Color::Rgb(0xee, 0xee, 0xee),
            muted,
            border,
            border_active: Color::Rgb(0x60, 0x60, 0x60),
            primary,
            secondary,
            accent,
            error: Color::Rgb(0xe0, 0x6c, 0x75),
            warning: Color::Rgb(0xf5, 0xa7, 0x42),
            success: Color::Rgb(0x7f, 0xd8, 0x8f),
            status_bg: Color::Rgb(0x0a, 0x0a, 0x0a),
            status_fg: muted,
            user_label: Style::default().fg(secondary).add_modifier(Modifier::BOLD),
            assistant_label: Style::default().fg(Color::Rgb(0xee, 0xee, 0xee)),
            system_label: Style::default().fg(muted),
            prompt_border: border_style,
            sidebar_border: border_style,
            dialog_border: border_style,
            dim: muted,
            toast: Style::default().fg(primary),
        }
    }

    pub fn light() -> Self {
        let primary = Color::Rgb(0x3b, 0x7d, 0xd8);
        let secondary = Color::Rgb(0x7b, 0x5b, 0xb6);
        let accent = Color::Rgb(0xd6, 0x8c, 0x27);
        let border = Color::Rgb(0xc8, 0xc8, 0xc8);
        let muted = Color::Rgb(0x8a, 0x8a, 0x8a);
        let border_style = Style::default().fg(border);
        Self {
            mode: ThemeMode::Light,
            bg: Color::Rgb(0xff, 0xff, 0xff),
            panel_bg: Color::Rgb(0xfa, 0xfa, 0xfa),
            fg: Color::Rgb(0x1a, 0x1a, 0x1a),
            muted,
            border,
            border_active: Color::Rgb(0x96, 0x96, 0x96),
            primary,
            secondary,
            accent,
            error: Color::Rgb(0xe0, 0x6c, 0x75),
            warning: Color::Rgb(0xf5, 0xa7, 0x42),
            success: Color::Rgb(0x3f, 0xa0, 0x5a),
            status_bg: Color::Rgb(0xff, 0xff, 0xff),
            status_fg: muted,
            user_label: Style::default().fg(secondary).add_modifier(Modifier::BOLD),
            assistant_label: Style::default().fg(Color::Rgb(0x1a, 0x1a, 0x1a)),
            system_label: Style::default().fg(muted),
            prompt_border: border_style,
            sidebar_border: border_style,
            dialog_border: border_style,
            dim: muted,
            toast: Style::default().fg(primary),
        }
    }
}
