import { vi, beforeEach } from 'vitest';
import { resetInvokeMocks, resetEventMocks, invoke, listen } from './mocks/tauri';

// Mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
  invoke,
}));

// Mock @tauri-apps/api/event
vi.mock('@tauri-apps/api/event', () => ({
  listen,
}));

// Mock @tauri-apps/plugin-fs
vi.mock('@tauri-apps/plugin-fs', () => ({
  exists: vi.fn(async () => true),
  watch: vi.fn(async () => () => {}),
}));

// Mock @tauri-apps/plugin-dialog
vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(async () => null),
  ask: vi.fn(async () => true),
}));

// Reset all mocks between tests
beforeEach(() => {
  resetInvokeMocks();
  resetEventMocks();
  vi.clearAllMocks();
});
