use gpui::{rgb, App, Hsla};

/// Catppuccin Mocha-inspired color palette — refined for minimalist polish.
pub struct Colors;

impl Colors {
    // ── Backgrounds ──────────────────────────────────────────────
    pub fn bg_base() -> Hsla { rgb(0x1e1e2e).into() }
    pub fn bg_surface() -> Hsla { rgb(0x282838).into() }     // slightly lighter than base
    pub fn bg_elevated() -> Hsla { rgb(0x313244).into() }    // cards, popovers
    pub fn bg_overlay() -> Hsla { rgb(0x45475a).into() }     // selection highlight

    // ── Text ─────────────────────────────────────────────────────
    pub fn text() -> Hsla { rgb(0xcdd6f4).into() }
    pub fn text_muted() -> Hsla { rgb(0x9399b2).into() }     // secondary text
    pub fn text_faint() -> Hsla { rgb(0x6c7086).into() }     // tertiary / disabled

    // ── Accent ───────────────────────────────────────────────────
    pub fn accent() -> Hsla { rgb(0x89b4fa).into() }
    pub fn accent_dim() -> Hsla { rgb(0x5d7cbf).into() }     // muted accent

    // ── Borders ──────────────────────────────────────────────────
    pub fn border() -> Hsla { rgb(0x3b3d52).into() }         // primary borders
    pub fn border_subtle() -> Hsla { rgb(0x313244).into() }  // subtle dividers

    // ── Semantic ─────────────────────────────────────────────────
    pub fn success() -> Hsla { rgb(0xa6e3a1).into() }
    pub fn warning() -> Hsla { rgb(0xf9e2af).into() }
    pub fn error() -> Hsla { rgb(0xf38ba8).into() }

    // ── Interactive ──────────────────────────────────────────────
    pub fn surface_hover() -> Hsla { rgb(0x313244).into() }
}

pub fn init(_app: &mut App) {
    // Future: register theme resources, load settings, etc.
}
