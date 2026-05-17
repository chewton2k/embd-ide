/**
 * Decide whether the editor should act on an `openFiles` store update
 * for a particular file.
 *
 * The store carries a `version` counter that's bumped by
 * `reloadFileContent` (after AI edit applies, file-watcher reloads,
 * git discard, etc.). The Editor's `$effect` listens for openFiles
 * changes and, when triggered by a fresh version, replaces the
 * editor doc with the store's content.
 *
 * Without per-path version tracking, the `$effect` would also fire on
 * unrelated store changes (e.g. `updateFileContent` flipping
 * `modified` true on the first keystroke). It would then see
 * `editorContent !== file.content` and dispatch a wholesale revert,
 * blowing away the user's typing. This helper tracks the last
 * version we acted on per file path so the effect only runs once
 * per actual reload event.
 */
export interface VersionedFile {
  version: number;
}

export function shouldDispatchVersionUpdate(
  file: VersionedFile | undefined,
  lastHandled: number,
): boolean {
  if (!file) return false;
  if (file.version === 0) return false;
  return file.version > lastHandled;
}
