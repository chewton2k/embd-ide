/**
 * Frontend wrapper for the Rust tree-sitter symbol extraction commands.
 *
 * Used by the agent's context builder to provide symbol-level retrieval
 * instead of full-file retrieval.
 */
import { invoke } from '@tauri-apps/api/core';

export interface Symbol {
  name: string;
  kind: string;
  start_line: number;
  end_line: number;
  body: string;
}

/**
 * Extract all symbols from a file.
 */
export async function extractSymbols(path: string): Promise<Symbol[]> {
  try {
    return await invoke<Symbol[]>('symbols_extract', { path });
  } catch {
    return [];
  }
}

/**
 * Get a specific symbol's body by name.
 */
export async function getSymbolBody(path: string, symbolName: string): Promise<string | null> {
  try {
    return await invoke<string>('symbols_get_body', { path, symbolName });
  } catch {
    return null;
  }
}
