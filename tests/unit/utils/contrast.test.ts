import { describe, expect, it } from 'vitest';
import {
  parseHexColor,
  compositeOver,
  relativeLuminance,
  contrastRatio,
  WCAG_AA_NORMAL_TEXT,
  WCAG_AA_LARGE_TEXT,
  WCAG_AA_NON_TEXT,
} from '$lib/modules/utils/contrast';

describe('parseHexColor', () => {
  it('parses #rrggbb', () => {
    expect(parseHexColor('#ffffff')).toEqual([255, 255, 255]);
    expect(parseHexColor('#000000')).toEqual([0, 0, 0]);
    expect(parseHexColor('#f0a020')).toEqual([0xf0, 0xa0, 0x20]);
  });

  it('parses #rgb shorthand', () => {
    expect(parseHexColor('#fff')).toEqual([255, 255, 255]);
    expect(parseHexColor('#abc')).toEqual([0xaa, 0xbb, 0xcc]);
  });

  it('parses #rrggbbaa (alpha ignored)', () => {
    expect(parseHexColor('#f0a02080')).toEqual([0xf0, 0xa0, 0x20]);
  });

  it('accepts colors without leading #', () => {
    expect(parseHexColor('ffffff')).toEqual([255, 255, 255]);
  });

  it('throws on invalid hex', () => {
    expect(() => parseHexColor('#xyz')).toThrow(/Invalid hex/);
    expect(() => parseHexColor('#1234')).toThrow(/Invalid hex/);
    expect(() => parseHexColor('not-a-color')).toThrow(/Invalid hex/);
  });
});

describe('compositeOver', () => {
  it('returns the foreground when alpha is 1', () => {
    expect(compositeOver([200, 100, 50, 1], [0, 0, 0])).toEqual([200, 100, 50]);
  });

  it('returns the background when alpha is 0', () => {
    expect(compositeOver([200, 100, 50, 0], [10, 20, 30])).toEqual([10, 20, 30]);
  });

  it('blends linearly at intermediate alpha', () => {
    // 50% of [200,200,200] over [0,0,0] = [100,100,100].
    expect(compositeOver([200, 200, 200, 0.5], [0, 0, 0])).toEqual([100, 100, 100]);
  });
});

describe('relativeLuminance', () => {
  it('white luminance is 1', () => {
    expect(relativeLuminance([255, 255, 255])).toBeCloseTo(1, 3);
  });

  it('black luminance is 0', () => {
    expect(relativeLuminance([0, 0, 0])).toBeCloseTo(0, 3);
  });

  it('grey is between black and white', () => {
    const grey = relativeLuminance([128, 128, 128]);
    expect(grey).toBeGreaterThan(0);
    expect(grey).toBeLessThan(1);
  });
});

describe('contrastRatio', () => {
  it('white-on-black is 21:1 (max)', () => {
    expect(contrastRatio([255, 255, 255], [0, 0, 0])).toBeCloseTo(21, 1);
  });

  it('same color is 1:1', () => {
    expect(contrastRatio([128, 128, 128], [128, 128, 128])).toBeCloseTo(1, 3);
  });

  it('is order-independent', () => {
    const a: [number, number, number] = [255, 255, 255];
    const b: [number, number, number] = [0, 0, 0];
    expect(contrastRatio(a, b)).toBeCloseTo(contrastRatio(b, a), 6);
  });
});

describe('app palette WCAG AA compliance', () => {
  // Editor surface background (var(--bg-surface) in Toast.svelte
  // fallback). Use hex form for the test.
  const surface = parseHexColor('#1e1e1e');
  // Primary text color (--text-primary) approximated.
  const textPrimary = parseHexColor('#e0e0e0');

  it('primary text on surface meets AA for normal text (≥4.5)', () => {
    expect(contrastRatio(textPrimary, surface)).toBeGreaterThanOrEqual(WCAG_AA_NORMAL_TEXT);
  });

  it('stale-badge text on its translucent overlay over the editor surface meets AA for normal text', () => {
    // Badge: color #ffcc66, background rgba(240,160,32,0.15) over #1e1e1e.
    // The originally-proposed #c97a00 only achieved 3.73:1 against the
    // overlay — failed AA. Brightening the foreground to a warmer
    // amber raises the ratio above the 4.5:1 threshold while keeping
    // the yellow-orange "caution" semantics.
    const badgeText = parseHexColor('#ffcc66');
    const overlay = compositeOver([0xf0, 0xa0, 0x20, 0.15], surface);
    expect(contrastRatio(badgeText, overlay)).toBeGreaterThanOrEqual(WCAG_AA_NORMAL_TEXT);
  });

  it('approve button text meets AA for large text (≥3.0)', () => {
    // Button: color #4ec9b0, background rgba(78,201,176,0.12) over surface.
    const btnText = parseHexColor('#4ec9b0');
    const btnBg = compositeOver([0x4e, 0xc9, 0xb0, 0.12], surface);
    expect(contrastRatio(btnText, btnBg)).toBeGreaterThanOrEqual(WCAG_AA_LARGE_TEXT);
  });

  it('reject button text meets AA for large text (≥3.0)', () => {
    const btnText = parseHexColor('#f14c4c');
    const btnBg = compositeOver([0xf1, 0x4c, 0x4c, 0.10], surface);
    expect(contrastRatio(btnText, btnBg)).toBeGreaterThanOrEqual(WCAG_AA_LARGE_TEXT);
  });

  it('stale-badge border vs surface meets AA for non-text UI components (≥3.0)', () => {
    // The yellow-orange border around the stale badge needs to be
    // distinguishable from the editor surface so users can identify
    // the warning chip at a glance. WCAG 1.4.11 (non-text contrast)
    // requires 3:1 against adjacent colors.
    const border = parseHexColor('#f0a020');
    expect(contrastRatio(border, surface)).toBeGreaterThanOrEqual(WCAG_AA_NON_TEXT);
  });
});
