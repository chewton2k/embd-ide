import { writable, type Writable } from 'svelte/store';

/**
 * Creates a Svelte writable store that automatically persists to localStorage.
 * Reads the initial value from localStorage (falling back to `defaultValue`),
 * and writes back on every change.
 */
export function persisted<T>(key: string, defaultValue: T, parse?: (raw: string) => T, serialize?: (val: T) => string): Writable<T> {
  const stored = localStorage.getItem(key);
  const initial = stored !== null && parse ? parse(stored) : (stored !== null ? stored as unknown as T : defaultValue);
  const store = writable<T>(initial);
  store.subscribe(v => {
    const s = serialize ? serialize(v) : String(v);
    if (v === '' || v === null || v === undefined) {
      localStorage.removeItem(key);
    } else {
      localStorage.setItem(key, s);
    }
  });
  return store;
}

/** Persisted string store. Empty string removes the key. */
export function persistedString(key: string, defaultValue = ''): Writable<string> {
  return persisted<string>(key, defaultValue, raw => raw, v => v);
}

/** Persisted number store. */
export function persistedNumber(key: string, defaultValue: number): Writable<number> {
  return persisted<number>(key, defaultValue, raw => parseInt(raw, 10) || defaultValue, v => String(v));
}

/** Persisted boolean store. */
export function persistedBool(key: string, defaultValue: boolean): Writable<boolean> {
  return persisted<boolean>(key, defaultValue, raw => raw === 'true', v => String(v));
}

/** Persisted boolean store where the default is true (stored as 'false' to disable). */
export function persistedBoolDefaultTrue(key: string): Writable<boolean> {
  return persisted<boolean>(key, true, raw => raw !== 'false', v => String(v));
}
