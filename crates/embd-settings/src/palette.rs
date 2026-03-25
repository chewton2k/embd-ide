use std::cell::RefCell;

/// A set of RGB hex color values defining a UI theme.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub bg_base: u32,
    pub bg_surface: u32,
    pub bg_elevated: u32,
    pub bg_overlay: u32,
    pub text: u32,
    pub text_muted: u32,
    pub text_faint: u32,
    pub accent: u32,
    pub accent_dim: u32,
    pub border: u32,
    pub border_subtle: u32,
    pub success: u32,
    pub warning: u32,
    pub error: u32,
    pub surface_hover: u32,
}

// ── Theme definitions ──────────────────────────────────────────────

const CATPPUCCIN_MOCHA: Palette = Palette {
    bg_base: 0x1e1e2e, bg_surface: 0x282838, bg_elevated: 0x313244, bg_overlay: 0x45475a,
    text: 0xcdd6f4, text_muted: 0x9399b2, text_faint: 0x6c7086,
    accent: 0x89b4fa, accent_dim: 0x5d7cbf,
    border: 0x3b3d52, border_subtle: 0x313244,
    success: 0xa6e3a1, warning: 0xf9e2af, error: 0xf38ba8, surface_hover: 0x313244,
};

const CATPPUCCIN_LATTE: Palette = Palette {
    bg_base: 0xeff1f5, bg_surface: 0xe6e9ef, bg_elevated: 0xdce0e8, bg_overlay: 0xccd0da,
    text: 0x4c4f69, text_muted: 0x6c6f85, text_faint: 0x8c8fa1,
    accent: 0x1e66f5, accent_dim: 0x7287fd,
    border: 0xbcc0cc, border_subtle: 0xccd0da,
    success: 0x40a02b, warning: 0xdf8e1d, error: 0xd20f39, surface_hover: 0xdce0e8,
};

const CATPPUCCIN_FRAPPE: Palette = Palette {
    bg_base: 0x303446, bg_surface: 0x3b3f54, bg_elevated: 0x414559, bg_overlay: 0x51576d,
    text: 0xc6d0f5, text_muted: 0xa5adce, text_faint: 0x838ba7,
    accent: 0x8caaee, accent_dim: 0x6c7cbf,
    border: 0x51576d, border_subtle: 0x414559,
    success: 0xa6d189, warning: 0xe5c890, error: 0xe78284, surface_hover: 0x414559,
};

const CATPPUCCIN_MACCHIATO: Palette = Palette {
    bg_base: 0x24273a, bg_surface: 0x2e3247, bg_elevated: 0x363a4f, bg_overlay: 0x494d64,
    text: 0xcad3f5, text_muted: 0xa5adcb, text_faint: 0x8087a2,
    accent: 0x8aadf4, accent_dim: 0x6a7dc4,
    border: 0x494d64, border_subtle: 0x363a4f,
    success: 0xa6da95, warning: 0xeed49f, error: 0xed8796, surface_hover: 0x363a4f,
};

const TOKYO_NIGHT: Palette = Palette {
    bg_base: 0x1a1b26, bg_surface: 0x222337, bg_elevated: 0x292e42, bg_overlay: 0x33394b,
    text: 0xc0caf5, text_muted: 0x9aa5ce, text_faint: 0x565f89,
    accent: 0x7aa2f7, accent_dim: 0x5a7cc4,
    border: 0x3b4261, border_subtle: 0x292e42,
    success: 0x9ece6a, warning: 0xe0af68, error: 0xf7768e, surface_hover: 0x292e42,
};

const TOKYO_NIGHT_STORM: Palette = Palette {
    bg_base: 0x24283b, bg_surface: 0x2b3045, bg_elevated: 0x333850, bg_overlay: 0x3b4261,
    text: 0xc0caf5, text_muted: 0x9aa5ce, text_faint: 0x565f89,
    accent: 0x7aa2f7, accent_dim: 0x5a7cc4,
    border: 0x3b4261, border_subtle: 0x333850,
    success: 0x9ece6a, warning: 0xe0af68, error: 0xf7768e, surface_hover: 0x333850,
};

const TOKYO_NIGHT_LIGHT: Palette = Palette {
    bg_base: 0xd5d6db, bg_surface: 0xcbccd1, bg_elevated: 0xc0c1c6, bg_overlay: 0xb4b5ba,
    text: 0x343b58, text_muted: 0x4c5374, text_faint: 0x6e7390,
    accent: 0x34548a, accent_dim: 0x5a6da4,
    border: 0xa9aab0, border_subtle: 0xc0c1c6,
    success: 0x485e30, warning: 0x8f5e15, error: 0x8c4351, surface_hover: 0xc0c1c6,
};

const DRACULA: Palette = Palette {
    bg_base: 0x282a36, bg_surface: 0x2e3040, bg_elevated: 0x343746, bg_overlay: 0x44475a,
    text: 0xf8f8f2, text_muted: 0xbfbfbf, text_faint: 0x6272a4,
    accent: 0xbd93f9, accent_dim: 0x9580c4,
    border: 0x44475a, border_subtle: 0x343746,
    success: 0x50fa7b, warning: 0xf1fa8c, error: 0xff5555, surface_hover: 0x343746,
};

const GRUVBOX_DARK: Palette = Palette {
    bg_base: 0x282828, bg_surface: 0x32302f, bg_elevated: 0x3c3836, bg_overlay: 0x504945,
    text: 0xebdbb2, text_muted: 0xbdae93, text_faint: 0x928374,
    accent: 0x83a598, accent_dim: 0x689d6a,
    border: 0x504945, border_subtle: 0x3c3836,
    success: 0xb8bb26, warning: 0xfabd2f, error: 0xfb4934, surface_hover: 0x3c3836,
};

const GRUVBOX_LIGHT: Palette = Palette {
    bg_base: 0xfbf1c7, bg_surface: 0xf2e5bc, bg_elevated: 0xebdbb2, bg_overlay: 0xd5c4a1,
    text: 0x3c3836, text_muted: 0x504945, text_faint: 0x928374,
    accent: 0x076678, accent_dim: 0x427b58,
    border: 0xbdae93, border_subtle: 0xd5c4a1,
    success: 0x79740e, warning: 0xb57614, error: 0x9d0006, surface_hover: 0xebdbb2,
};

const NORD: Palette = Palette {
    bg_base: 0x2e3440, bg_surface: 0x353b49, bg_elevated: 0x3b4252, bg_overlay: 0x434c5e,
    text: 0xeceff4, text_muted: 0xd8dee9, text_faint: 0x7b88a1,
    accent: 0x88c0d0, accent_dim: 0x81a1c1,
    border: 0x434c5e, border_subtle: 0x3b4252,
    success: 0xa3be8c, warning: 0xebcb8b, error: 0xbf616a, surface_hover: 0x3b4252,
};

const SOLARIZED_DARK: Palette = Palette {
    bg_base: 0x002b36, bg_surface: 0x073642, bg_elevated: 0x0a3f4d, bg_overlay: 0x1a4f5e,
    text: 0x839496, text_muted: 0x657b83, text_faint: 0x586e75,
    accent: 0x268bd2, accent_dim: 0x2aa198,
    border: 0x1a4f5e, border_subtle: 0x0a3f4d,
    success: 0x859900, warning: 0xb58900, error: 0xdc322f, surface_hover: 0x0a3f4d,
};

const SOLARIZED_LIGHT: Palette = Palette {
    bg_base: 0xfdf6e3, bg_surface: 0xeee8d5, bg_elevated: 0xe4ddca, bg_overlay: 0xd6cdb9,
    text: 0x657b83, text_muted: 0x839496, text_faint: 0x93a1a1,
    accent: 0x268bd2, accent_dim: 0x2aa198,
    border: 0xc9c2ad, border_subtle: 0xd6cdb9,
    success: 0x859900, warning: 0xb58900, error: 0xdc322f, surface_hover: 0xe4ddca,
};

const ONE_DARK: Palette = Palette {
    bg_base: 0x282c34, bg_surface: 0x2e3239, bg_elevated: 0x353a42, bg_overlay: 0x3e4451,
    text: 0xabb2bf, text_muted: 0x8b929e, text_faint: 0x636d83,
    accent: 0x61afef, accent_dim: 0x4d8ac4,
    border: 0x3e4451, border_subtle: 0x353a42,
    success: 0x98c379, warning: 0xe5c07b, error: 0xe06c75, surface_hover: 0x353a42,
};

const ONE_LIGHT: Palette = Palette {
    bg_base: 0xfafafa, bg_surface: 0xf0f0f0, bg_elevated: 0xe5e5e5, bg_overlay: 0xd4d4d4,
    text: 0x383a42, text_muted: 0x696c77, text_faint: 0xa0a1a7,
    accent: 0x4078f2, accent_dim: 0x526fff,
    border: 0xc8c8c8, border_subtle: 0xd4d4d4,
    success: 0x50a14f, warning: 0xc18401, error: 0xe45649, surface_hover: 0xe5e5e5,
};

const ROSE_PINE: Palette = Palette {
    bg_base: 0x191724, bg_surface: 0x211f2e, bg_elevated: 0x26233a, bg_overlay: 0x312e47,
    text: 0xe0def4, text_muted: 0x908caa, text_faint: 0x6e6a86,
    accent: 0xc4a7e7, accent_dim: 0x9ccfd8,
    border: 0x3a374e, border_subtle: 0x26233a,
    success: 0x31748f, warning: 0xf6c177, error: 0xeb6f92, surface_hover: 0x26233a,
};

const ROSE_PINE_MOON: Palette = Palette {
    bg_base: 0x232136, bg_surface: 0x2a273f, bg_elevated: 0x393552, bg_overlay: 0x44415a,
    text: 0xe0def4, text_muted: 0x908caa, text_faint: 0x6e6a86,
    accent: 0xc4a7e7, accent_dim: 0x9ccfd8,
    border: 0x44415a, border_subtle: 0x393552,
    success: 0x3e8fb0, warning: 0xf6c177, error: 0xeb6f92, surface_hover: 0x393552,
};

const ROSE_PINE_DAWN: Palette = Palette {
    bg_base: 0xfaf4ed, bg_surface: 0xf2e9de, bg_elevated: 0xe8ddd3, bg_overlay: 0xdbd1c5,
    text: 0x575279, text_muted: 0x797593, text_faint: 0x9893a5,
    accent: 0x907aa9, accent_dim: 0x56949f,
    border: 0xcec5b4, border_subtle: 0xdbd1c5,
    success: 0x286983, warning: 0xea9d34, error: 0xb4637a, surface_hover: 0xe8ddd3,
};

const MONOKAI: Palette = Palette {
    bg_base: 0x272822, bg_surface: 0x2d2e27, bg_elevated: 0x3e3d32, bg_overlay: 0x49483e,
    text: 0xf8f8f2, text_muted: 0xc0c0b0, text_faint: 0x75715e,
    accent: 0x66d9ef, accent_dim: 0xa6e22e,
    border: 0x49483e, border_subtle: 0x3e3d32,
    success: 0xa6e22e, warning: 0xe6db74, error: 0xf92672, surface_hover: 0x3e3d32,
};

const GITHUB_DARK: Palette = Palette {
    bg_base: 0x0d1117, bg_surface: 0x161b22, bg_elevated: 0x1c2129, bg_overlay: 0x21262d,
    text: 0xc9d1d9, text_muted: 0x8b949e, text_faint: 0x6e7681,
    accent: 0x58a6ff, accent_dim: 0x388bfd,
    border: 0x30363d, border_subtle: 0x21262d,
    success: 0x3fb950, warning: 0xd29922, error: 0xf85149, surface_hover: 0x1c2129,
};

const GITHUB_LIGHT: Palette = Palette {
    bg_base: 0xffffff, bg_surface: 0xf6f8fa, bg_elevated: 0xeaeef2, bg_overlay: 0xd8dee4,
    text: 0x24292f, text_muted: 0x57606a, text_faint: 0x8c959f,
    accent: 0x0969da, accent_dim: 0x0550ae,
    border: 0xd0d7de, border_subtle: 0xd8dee4,
    success: 0x1a7f37, warning: 0x9a6700, error: 0xcf222e, surface_hover: 0xeaeef2,
};

// ── Active palette (thread-local) ──────────────────────────────────

thread_local! {
    static ACTIVE: RefCell<Palette> = RefCell::new(CATPPUCCIN_MOCHA);
}

/// Look up a palette by theme ID string.
pub fn palette_for_id(id: &str) -> Palette {
    match id {
        "catppuccin-mocha" => CATPPUCCIN_MOCHA,
        "catppuccin-latte" => CATPPUCCIN_LATTE,
        "catppuccin-frappe" => CATPPUCCIN_FRAPPE,
        "catppuccin-macchiato" => CATPPUCCIN_MACCHIATO,
        "tokyo-night" => TOKYO_NIGHT,
        "tokyo-night-storm" => TOKYO_NIGHT_STORM,
        "tokyo-night-light" => TOKYO_NIGHT_LIGHT,
        "dracula" => DRACULA,
        "gruvbox-dark" => GRUVBOX_DARK,
        "gruvbox-light" => GRUVBOX_LIGHT,
        "nord" => NORD,
        "solarized-dark" => SOLARIZED_DARK,
        "solarized-light" => SOLARIZED_LIGHT,
        "one-dark" => ONE_DARK,
        "one-light" => ONE_LIGHT,
        "rose-pine" => ROSE_PINE,
        "rose-pine-moon" => ROSE_PINE_MOON,
        "rose-pine-dawn" => ROSE_PINE_DAWN,
        "monokai" => MONOKAI,
        "github-dark" => GITHUB_DARK,
        "github-light" => GITHUB_LIGHT,
        _ => CATPPUCCIN_MOCHA,
    }
}

/// Set the active color theme by ID.
pub fn set_theme(id: &str) {
    let p = palette_for_id(id);
    ACTIVE.with(|a| *a.borrow_mut() = p);
}

/// Get the currently active palette.
pub fn active_palette() -> Palette {
    ACTIVE.with(|a| *a.borrow())
}
