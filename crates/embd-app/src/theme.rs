use gpui::{rgb, App, Hsla};
use embd_settings::palette;

pub struct Colors;

impl Colors {
    fn p() -> palette::Palette { palette::active_palette() }

    // ── Backgrounds ──────────────────────────────────────────────
    pub fn bg_base() -> Hsla { rgb(Self::p().bg_base).into() }
    pub fn bg_surface() -> Hsla { rgb(Self::p().bg_surface).into() }
    pub fn bg_elevated() -> Hsla { rgb(Self::p().bg_elevated).into() }
    pub fn bg_overlay() -> Hsla { rgb(Self::p().bg_overlay).into() }

    // ── Text ─────────────────────────────────────────────────────
    pub fn text() -> Hsla { rgb(Self::p().text).into() }
    pub fn text_muted() -> Hsla { rgb(Self::p().text_muted).into() }
    pub fn text_faint() -> Hsla { rgb(Self::p().text_faint).into() }

    // ── Accent ───────────────────────────────────────────────────
    pub fn accent() -> Hsla { rgb(Self::p().accent).into() }
    pub fn accent_dim() -> Hsla { rgb(Self::p().accent_dim).into() }

    // ── Borders ──────────────────────────────────────────────────
    pub fn border() -> Hsla { rgb(Self::p().border).into() }
    pub fn border_subtle() -> Hsla { rgb(Self::p().border_subtle).into() }

    // ── Semantic ─────────────────────────────────────────────────
    pub fn success() -> Hsla { rgb(Self::p().success).into() }
    pub fn warning() -> Hsla { rgb(Self::p().warning).into() }
    pub fn error() -> Hsla { rgb(Self::p().error).into() }

    // ── Interactive ──────────────────────────────────────────────
    pub fn surface_hover() -> Hsla { rgb(Self::p().surface_hover).into() }
}

pub fn init(_app: &mut App) {
    // Load theme from saved settings
    let settings = embd_settings::Settings::load(&embd_settings::Settings::default_path());
    palette::set_theme(&settings.appearance.theme_id);
}
