import { invoke } from '@tauri-apps/api/core';

export async function openNewWindow(initialProject?: string): Promise<string> {
  return invoke<string>('open_new_window', { initialProject: initialProject ?? null });
}

export async function openFolderInNewWindow(path: string): Promise<string> {
  return invoke<string>('open_folder_in_new_window', { path });
}

export async function closeWindow(): Promise<void> {
  return invoke<void>('close_focused_window');
}
