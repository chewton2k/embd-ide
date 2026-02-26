# Themes

embd ships with 12 built-in themes. You can also add your own by editing the source.

## Adding a Custom Theme

1. Open `src/lib/stores.ts`
2. Find the `THEMES` array (around line 275)
3. Add a new entry to the array:

```ts
{
  id: 'my-theme',        // unique kebab-case ID
  name: 'My Theme',      // display name shown in settings
  colors: {
    // UI Colors
    bgPrimary:     '#1e1e2e',
    bgSecondary:   '#181825',
    bgTertiary:    '#11111b',
    bgSurface:     '#313244',
    textPrimary:   '#cdd6f4',
    textSecondary: '#a6adc8',
    textMuted:     '#6c7086',
    accent:        '#89b4fa',
    accentHover:   '#74c7ec',
    border:        '#45475a',
    success:       '#a6e3a1',
    warning:       '#f9e2af',
    error:         '#f38ba8',

    // Terminal Colors
    termBg:          '#11111b',
    termFg:          '#cdd6f4',
    termCursor:      '#89b4fa',
    termSelection:   '#45475a',
    termBlack:       '#45475a',
    termRed:         '#f38ba8',
    termGreen:       '#a6e3a1',
    termYellow:      '#f9e2af',
    termBlue:        '#89b4fa',
    termMagenta:     '#cba6f7',
    termCyan:        '#94e2d5',
    termWhite:       '#bac2de',
    termBrightBlack: '#585b70',
    termBrightWhite: '#a6adc8',

    // Optional — these fall back to accent/success/error if omitted
    // gitGraphAccent:  '#e8a45f',
    // diffAdd:         '#a6e3a1',
    // diffDel:         '#f38ba8',
    // gitNotification: '#e8a45f',
  },
},
```

4. Rebuild/restart the app. Your theme will appear in **Settings > Theme**.

## Color Reference

All values are hex strings. Every color below is **required** unless marked optional.

### UI Colors

| Key | What it controls |
|---|---|
| `bgPrimary` | Main editor and panel background |
| `bgSecondary` | Sidebar, secondary panels, inactive tabs |
| `bgTertiary` | Deepest background layer (behind everything) |
| `bgSurface` | Elevated elements — dropdowns, tooltips, hover highlights |
| `textPrimary` | Main text (filenames, code, headings) |
| `textSecondary` | Secondary text (descriptions, labels) |
| `textMuted` | Dimmest text (placeholders, disabled items, line numbers) |
| `accent` | Primary interactive color — selected tab underline, active buttons, links, focus rings |
| `accentHover` | Accent color on hover/active state |
| `border` | All borders and dividers between panels |
| `success` | Git added indicators, success messages |
| `warning` | Git modified indicators, warning messages |
| `error` | Git deleted indicators, error messages |

### Terminal Colors

These map directly to ANSI terminal colors used by the integrated terminal.

| Key | What it controls |
|---|---|
| `termBg` | Terminal background |
| `termFg` | Terminal default text |
| `termCursor` | Cursor color |
| `termSelection` | Selected text background |
| `termBlack` | ANSI black |
| `termRed` | ANSI red (errors, failed commands) |
| `termGreen` | ANSI green (success, diffs added) |
| `termYellow` | ANSI yellow (warnings) |
| `termBlue` | ANSI blue (directories in `ls`, info) |
| `termMagenta` | ANSI magenta (special files) |
| `termCyan` | ANSI cyan (symlinks, strings) |
| `termWhite` | ANSI white (default bright text) |
| `termBrightBlack` | Bright black (comments, muted terminal text) |
| `termBrightWhite` | Bright white (emphasized terminal text) |

### Optional Git Overrides

These are optional. If omitted, they fall back to the values shown:

| Key | Fallback | What it controls |
|---|---|---|
| `gitGraphAccent` | `accent` | Branch lines and dots in the git graph |
| `diffAdd` | `success` | Added lines in diffs |
| `diffDel` | `error` | Deleted lines in diffs |
| `gitNotification` | `success` | Git notification badge/indicator |

## Tips

- Start by duplicating an existing theme that's close to what you want and tweaking the values.
- Keep `bgPrimary`, `bgSecondary`, `bgTertiary` close in hue but stepping in brightness — this creates the layered panel look.
- Make sure `textPrimary` has strong contrast against `bgPrimary` for readability.
- For a monochrome look (like the Ash or Midnight themes), you can use grayscale values for `success`, `warning`, and `error` — but keep in mind this reduces visual distinction for git status indicators. Use the optional git overrides to bring color back selectively.
- `termBg` is typically set to `bgTertiary` so the terminal blends into the deepest layer.
