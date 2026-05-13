import { invoke } from '@tauri-apps/api/core';

type LogLevel = 'debug' | 'info' | 'warn' | 'error';

const SECRET_PATTERN = /api[_-]?key|token|password|secret/i;
const MAX_FIELD_SIZE = 8192;

function redactSecrets(data: unknown): unknown {
  if (data === null || data === undefined) return data;
  if (typeof data !== 'object') return data;
  if (Array.isArray(data)) return data.map(redactSecrets);
  const result: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(data as Record<string, unknown>)) {
    if (SECRET_PATTERN.test(key) && typeof value === 'string') {
      result[key] = '[redacted]';
    } else if (typeof value === 'object' && value !== null) {
      result[key] = redactSecrets(value);
    } else {
      result[key] = value;
    }
  }
  return result;
}

function truncate(value: string): string {
  return value.length > MAX_FIELD_SIZE ? value.slice(0, MAX_FIELD_SIZE) + '…[truncated]' : value;
}

function shouldSample(level: LogLevel): boolean {
  if (level === 'debug' && !import.meta.env.DEV) {
    return Math.random() < 0.01;
  }
  return true;
}

function sendToBackend(level: LogLevel, msg: string, data?: unknown, err?: { message: string; stack?: string }) {
  try {
    const payload = {
      level,
      msg: truncate(msg),
      ts: Date.now(),
      data: data !== undefined ? truncate(JSON.stringify(redactSecrets(data))) : undefined,
      err: err ? { message: truncate(err.message), stack: err.stack ? truncate(err.stack) : undefined } : undefined,
    };
    invoke('log_record', payload).catch(() => {});
  } catch {
    // Logger must never crash the app
  }
}

function createLogger(scope?: string) {
  const prefix = scope ? `[${scope}] ` : '';

  return {
    debug(msg: string, data?: unknown) {
      if (!shouldSample('debug')) return;
      const fullMsg = prefix + msg;
      if (import.meta.env.DEV) console.debug(fullMsg, data ?? '');
      sendToBackend('debug', fullMsg, data);
    },
    info(msg: string, data?: unknown) {
      const fullMsg = prefix + msg;
      if (import.meta.env.DEV) console.info(fullMsg, data ?? '');
      sendToBackend('info', fullMsg, data);
    },
    warn(msg: string, data?: unknown) {
      const fullMsg = prefix + msg;
      if (import.meta.env.DEV) console.warn(fullMsg, data ?? '');
      sendToBackend('warn', fullMsg, data);
    },
    error(msg: string, err?: unknown, data?: unknown) {
      const fullMsg = prefix + msg;
      const errObj = err instanceof Error
        ? { message: err.message, stack: err.stack }
        : err !== undefined
          ? { message: String(err), stack: undefined }
          : undefined;
      if (import.meta.env.DEV) console.error(fullMsg, err ?? '', data ?? '');
      sendToBackend('error', fullMsg, data, errObj);
    },
    scope(name: string) {
      return createLogger(scope ? `${scope}:${name}` : name);
    },
  };
}

export const log = createLogger();
