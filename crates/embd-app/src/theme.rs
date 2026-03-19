use gpui::{rgb, App, Hsla};

/// Catppuccin Mocha-inspired color palette.
pub struct Colors;

impl Colors {
    pub fn bg_base() -> Hsla { rgb(0x1e1e2e).into() }
    pub fn bg_surface() -> Hsla { rgb(0x313244).into() }
    pub fn bg_overlay() -> Hsla { rgb(0x45475a).into() }
    pub fn text() -> Hsla { rgb(0xcdd6f4).into() }
    pub fn text_muted() -> Hsla { rgb(0xa6adc8).into() }
    pub fn accent() -> Hsla { rgb(0x89b4fa).into() }
    pub fn border() -> Hsla { rgb(0x585b70).into() }
    pub fn success() -> Hsla { rgb(0xa6e3a1).into() }
    pub fn warning() -> Hsla { rgb(0xf9e2af).into() }
    pub fn error() -> Hsla { rgb(0xf38ba8).into() }
    pub fn surface_hover() -> Hsla { rgb(0x3b3d52).into() }
}

pub fn init(_app: &mut App) {
    // Future: register theme resources, load settings, etc.
}
