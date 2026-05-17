/**
 * WCAG 2.1 contrast-ratio utilities.
 *
 * Used to programmatically assert that our chosen color palette meets
 * accessibility thresholds, so design changes can't silently regress
 * legibility for low-vision users.
 */

/** Parse `#rgb`, `#rrggbb`, or `#rrggbbaa` into [r, g, b] 0..255 (alpha ignored). */
export function parseHexColor(hex: string): [number, number, number] {
  let h = hex.trim().replace(/^#/, '');
  if (h.length === 3) {
    h = h.split('').map(c => c + c).join('');
  }
  if (h.length !== 6 && h.length !== 8) {
    throw new Error(`Invalid hex color: "${hex}"`);
  }
  const r = parseInt(h.slice(0, 2), 16);
  const g = parseInt(h.slice(2, 4), 16);
  const b = parseInt(h.slice(4, 6), 16);
  if (Number.isNaN(r) || Number.isNaN(g) || Number.isNaN(b)) {
    throw new Error(`Invalid hex color: "${hex}"`);
  }
  return [r, g, b];
}

/**
 * Composite an RGBA foreground over an opaque RGB background and
 * return the resulting opaque RGB. Needed because our stale tints
 * use translucent overlays (`rgba(240,160,32,0.15)` etc.) that read
 * differently depending on the background they sit on.
 */
export function compositeOver(
  fg: [number, number, number, number],
  bg: [number, number, number],
): [number, number, number] {
  const [fr, fg_, fb, fa] = fg;
  const [br, bg2, bb] = bg;
  return [
    Math.round(fr * fa + br * (1 - fa)),
    Math.round(fg_ * fa + bg2 * (1 - fa)),
    Math.round(fb * fa + bb * (1 - fa)),
  ];
}

/**
 * Per WCAG 2.1 §1.4.3 — relative luminance of an sRGB color.
 * Inputs are 0..255 channel values.
 */
export function relativeLuminance([r, g, b]: [number, number, number]): number {
  const channel = (c: number): number => {
    const v = c / 255;
    return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
  };
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

/**
 * WCAG 2.1 contrast ratio between two opaque colors (1.0 to 21.0).
 * Order-independent.
 */
export function contrastRatio(
  a: [number, number, number],
  b: [number, number, number],
): number {
  const la = relativeLuminance(a);
  const lb = relativeLuminance(b);
  const [hi, lo] = la > lb ? [la, lb] : [lb, la];
  return (hi + 0.05) / (lo + 0.05);
}

/** WCAG AA threshold for normal-size text (≥4.5:1). */
export const WCAG_AA_NORMAL_TEXT = 4.5;
/** WCAG AA threshold for large text (≥3:1). 18pt+ or 14pt+ bold. */
export const WCAG_AA_LARGE_TEXT = 3.0;
/** WCAG AA threshold for non-text UI components and graphical objects (≥3:1). */
export const WCAG_AA_NON_TEXT = 3.0;
