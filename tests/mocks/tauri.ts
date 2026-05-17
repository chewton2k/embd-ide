import { vi } from 'vitest';

type InvokeHandler = (args: unknown) => unknown;

const invokeHandlers = new Map<string, InvokeHandler>();
const invokeCalls: Array<{ cmd: string; args: unknown }> = [];

export function mockInvoke(cmd: string, handler: InvokeHandler): void {
  invokeHandlers.set(cmd, handler);
}

export function expectInvoked(cmd: string, args?: unknown): void {
  const call = invokeCalls.find(c => c.cmd === cmd);
  if (!call) throw new Error(`Expected invoke('${cmd}') to have been called, but it was not.`);
  if (args !== undefined) {
    const match = invokeCalls.find(c => c.cmd === cmd && JSON.stringify(c.args) === JSON.stringify(args));
    if (!match) throw new Error(`invoke('${cmd}') was called but not with expected args: ${JSON.stringify(args)}`);
  }
}

export function getInvokeCalls(cmd?: string) {
  return cmd ? invokeCalls.filter(c => c.cmd === cmd) : [...invokeCalls];
}

export function resetInvokeMocks(): void {
  invokeHandlers.clear();
  invokeCalls.length = 0;
}

// The mock invoke function that replaces @tauri-apps/api/core::invoke
export const invoke = vi.fn(async (cmd: string, args?: unknown) => {
  invokeCalls.push({ cmd, args });
  const handler = invokeHandlers.get(cmd);
  if (!handler) {
    throw new Error(`Unmocked Tauri command: '${cmd}'. Register a handler with mockInvoke('${cmd}', handler).`);
  }
  return handler(args);
});

// Mock for @tauri-apps/api/event::listen
type EventCallback = (event: { payload: unknown }) => void;
const eventListeners = new Map<string, EventCallback[]>();

export const listen = vi.fn(async (event: string, handler: EventCallback) => {
  const listeners = eventListeners.get(event) || [];
  listeners.push(handler);
  eventListeners.set(event, listeners);
  return () => {
    const idx = listeners.indexOf(handler);
    if (idx >= 0) listeners.splice(idx, 1);
  };
});

export function emitEvent(event: string, payload: unknown): void {
  const listeners = eventListeners.get(event) || [];
  for (const handler of listeners) {
    handler({ payload });
  }
}

export function resetEventMocks(): void {
  eventListeners.clear();
}
