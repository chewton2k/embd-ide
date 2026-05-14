/**
 * Persisted allow-list of hostnames the user has approved for inline
 * preview rendering.
 *
 * Why an allow-list at all? The Tauri CSP `frame-src` directive used to
 * include `https://*`, which let *any* HTTPS host render inside the
 * preview iframe. That's an avoidable footgun: a typo in the URL bar,
 * or a copy-pasted internal URL, could load arbitrary third-party
 * pages with credentials in the embedding context. The allow-list +
 * tightened CSP (see tauri.conf.json `frame-src`) means previews are
 * gated:
 *
 *   - localhost / 127.0.0.1 (any port, any protocol): always allowed
 *   - hosts in the persisted allow-list: allowed
 *   - everything else: prompted
 *
 * The allow-list is persisted to localStorage as a JSON array of
 * hostnames so it survives across IDE restarts.
 *
 * Hostnames are stored normalized: lowercase, no port/path. We compare
 * with `URL(target).hostname`, which has the same shape.
 */

import { derived, get, type Readable } from 'svelte/store';
import { persistedString } from '../session/persisted';

const STORAGE_KEY = 'leo-preview-allowlist';

/** Underlying persisted JSON-array string. */
const raw = persistedString(STORAGE_KEY, '[]');

function parse(rawStr: string): string[] {
  try {
    const v = JSON.parse(rawStr);
    if (!Array.isArray(v)) return [];
    return v.filter((x): x is string => typeof x === 'string').map((h) => h.toLowerCase());
  } catch {
    return [];
  }
}

/** Read-only view of the current allow-list (lowercased hostnames). */
export const previewAllowList: Readable<string[]> = derived(raw, ($r) => parse($r));

/**
 * Returns true when `target` is either a loopback address or a host
 * the user has previously approved. Falsy / unparseable inputs return
 * false.
 */
export function isAllowed(target: string | null | undefined): boolean {
  if (!target) return false;
  let url: URL;
  try {
    url = new URL(target);
  } catch {
    return false;
  }
  if (isLocalHost(url.hostname)) return true;
  const list = parse(get(raw));
  return list.includes(url.hostname.toLowerCase());
}

/** True for the standard set of loopback hostnames. */
export function isLocalHost(hostname: string): boolean {
  const h = hostname.toLowerCase();
  return (
    h === 'localhost' ||
    h === '127.0.0.1' ||
    h === '0.0.0.0' ||
    h === '[::1]' ||
    h === '::1'
  );
}

/**
 * Add a host to the allow-list (idempotent). Accepts a full URL or a
 * raw hostname; the URL form is normalized to its `hostname`. Returns
 * `true` when the list actually changed.
 */
export function addToAllowList(hostOrUrl: string): boolean {
  const host = normalize(hostOrUrl);
  if (!host) return false;
  const current = parse(get(raw));
  if (current.includes(host)) return false;
  const next = [...current, host].sort();
  raw.set(JSON.stringify(next));
  return true;
}

/** Remove a host from the allow-list. Returns `true` if it was present. */
export function removeFromAllowList(hostOrUrl: string): boolean {
  const host = normalize(hostOrUrl);
  if (!host) return false;
  const current = parse(get(raw));
  if (!current.includes(host)) return false;
  raw.set(JSON.stringify(current.filter((h) => h !== host)));
  return true;
}

/** Wipe the allow-list. Mostly useful for tests / settings. */
export function clearAllowList(): void {
  raw.set('[]');
}

function normalize(input: string): string | null {
  const s = (input ?? '').trim();
  if (!s) return null;
  // If it looks like a URL, parse and take its hostname; otherwise
  // treat the whole string as a bare hostname.
  if (/^[a-z][a-z0-9+.-]*:\/\//i.test(s)) {
    try {
      return new URL(s).hostname.toLowerCase() || null;
    } catch {
      return null;
    }
  }
  // Bare host. Strip trailing slashes and any `:port` suffix so the
  // result lines up with the `URL.hostname` shape that `isAllowed`
  // compares against.
  const stripped = s.replace(/\/+$/, '').toLowerCase();
  // Only strip port when the part before ':' is itself a hostname-like
  // string. Bracketed IPv6 (`[::1]:8080`) → split on the closing bracket.
  if (stripped.startsWith('[')) {
    const end = stripped.indexOf(']');
    if (end > 0) return stripped.slice(0, end + 1) || null;
  }
  const colon = stripped.indexOf(':');
  if (colon > 0) {
    const host = stripped.slice(0, colon);
    if (host) return host;
  }
  return stripped || null;
}
