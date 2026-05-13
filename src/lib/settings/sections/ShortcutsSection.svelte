<script lang="ts">
  import SectionHeader from '../components/SectionHeader.svelte';
  import { RotateCcw, Trash2, AlertTriangle } from 'lucide-svelte';
  import {
    DEFAULT_SHORTCUTS,
    shortcutBindings,
    setShortcut,
    removeShortcut,
    resetShortcut,
    resetAllShortcuts,
    findConflicts,
    normalizeKeyEvent,
    formatKeysForDisplay,
    type ShortcutDef,
  } from '../../modules/shortcuts';

  // Group shortcuts by their group field
  const groups = $derived.by(() => {
    const map = new Map<string, ShortcutDef[]>();
    for (const def of DEFAULT_SHORTCUTS) {
      if (!map.has(def.group)) map.set(def.group, []);
      map.get(def.group)!.push(def);
    }
    return [...map.entries()];
  });

  let recordingId = $state<string | null>(null);
  let recordedKeys = $state('');
  let conflicts = $state<ShortcutDef[]>([]);

  function startRecording(id: string) {
    recordingId = id;
    recordedKeys = '';
    conflicts = [];
  }

  function cancelRecording() {
    recordingId = null;
    recordedKeys = '';
    conflicts = [];
  }

  function handleRecordKey(e: KeyboardEvent) {
    if (!recordingId) return;

    if (e.key === 'Escape') {
      e.preventDefault();
      cancelRecording();
      return;
    }

    const normalized = normalizeKeyEvent(e);
    if (!normalized) return; // standalone modifier — don't block

    e.preventDefault();
    e.stopPropagation();
    recordedKeys = normalized;
    conflicts = findConflicts(normalized, recordingId);
  }

  function confirmBinding() {
    if (!recordingId || !recordedKeys) return;
    setShortcut(recordingId, recordedKeys);
    recordingId = null;
    recordedKeys = '';
    conflicts = [];
  }

  function handleDelete(id: string) {
    removeShortcut(id);
  }

  function handleReset(id: string) {
    resetShortcut(id);
  }

  function handleResetAll() {
    resetAllShortcuts();
  }

  const hasAnyModified = $derived(
    DEFAULT_SHORTCUTS.some(d => $shortcutBindings[d.id] !== d.defaultKeys)
  );
</script>

<svelte:window onkeydown={handleRecordKey} />

<div class="root" data-setting="shortcuts">
  <SectionHeader
    title="Shortcuts"
    description="Customize keyboard shortcuts. Click a binding to reassign, or use the actions to delete or reset."
  />

  <div class="toolbar">
    <button
      class="reset-all-btn"
      onclick={handleResetAll}
      disabled={!hasAnyModified}
      title="Reset all shortcuts to defaults"
    >
      <RotateCcw size={12} />
      <span>Reset all to defaults</span>
    </button>
  </div>

  {#each groups as [groupName, items]}
    <div class="card">
      <div class="card-head">{groupName}</div>
      <div class="list">
        {#each items as item}
          {@const binding = $shortcutBindings[item.id] ?? ''}
          {@const modified = binding !== item.defaultKeys}
          {@const isRecording = recordingId === item.id}
          <div class="row" class:recording={isRecording} class:modified>
            <span class="label">{item.label}</span>
            <div class="binding-area">
              {#if isRecording}
                <div class="recorder">
                  <kbd class="recording-kbd">
                    {recordedKeys ? formatKeysForDisplay(recordedKeys) : 'Press keys…'}
                  </kbd>
                  {#if conflicts.length > 0}
                    <div class="conflict">
                      <AlertTriangle size={11} />
                      <span>Conflicts with: {conflicts.map(c => c.label).join(', ')}</span>
                    </div>
                  {/if}
                  <div class="recorder-actions">
                    <button
                      class="rec-btn confirm"
                      onclick={confirmBinding}
                      disabled={!recordedKeys}
                      title="Confirm binding"
                    >Apply</button>
                    <button
                      class="rec-btn cancel"
                      onclick={cancelRecording}
                      title="Cancel (Esc)"
                    >Cancel</button>
                  </div>
                </div>
              {:else}
                <button
                  class="kbd-btn"
                  class:unbound={!binding}
                  onclick={() => startRecording(item.id)}
                  title="Click to reassign"
                >
                  {formatKeysForDisplay(binding)}
                </button>
                <div class="row-actions">
                  <button
                    class="icon-btn"
                    onclick={() => handleDelete(item.id)}
                    disabled={!binding}
                    title="Remove binding"
                    aria-label="Remove binding"
                  >
                    <Trash2 size={12} />
                  </button>
                  <button
                    class="icon-btn"
                    onclick={() => handleReset(item.id)}
                    disabled={!modified}
                    title="Reset to default"
                    aria-label="Reset to default"
                  >
                    <RotateCcw size={12} />
                  </button>
                </div>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    </div>
  {/each}
</div>

<style>
  .root { display: flex; flex-direction: column; gap: 16px; }

  .toolbar {
    display: flex;
    justify-content: flex-end;
  }
  .reset-all-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px;
    font-size: 11.5px;
    font-weight: 500;
    color: var(--text-secondary);
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    cursor: pointer;
    transition: background 0.12s, color 0.12s, border-color 0.12s;
  }
  .reset-all-btn:hover:not(:disabled) {
    background: var(--bg-tertiary);
    color: var(--text-primary);
    border-color: color-mix(in srgb, var(--accent) 30%, var(--border));
  }
  .reset-all-btn:disabled { opacity: 0.4; cursor: not-allowed; }

  .card {
    background: color-mix(in srgb, var(--bg-tertiary) 60%, transparent);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 14px 16px;
    transition: border-color 0.15s ease;
  }
  .card:hover {
    border-color: color-mix(in srgb, var(--accent) 18%, var(--border));
  }
  .card-head {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
    margin-bottom: 8px;
    letter-spacing: -0.1px;
  }
  .list { display: flex; flex-direction: column; }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 0;
    gap: 12px;
  }
  .row + .row { border-top: 1px solid color-mix(in srgb, var(--border) 60%, transparent); }
  .row.recording {
    background: color-mix(in srgb, var(--accent) 6%, transparent);
    border-radius: 6px;
    padding: 10px 8px;
    margin: 0 -8px;
  }
  .row.modified .label {
    color: var(--accent);
  }

  .label { font-size: 13px; color: var(--text-primary); font-weight: 500; flex-shrink: 0; }

  .binding-area {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .kbd-btn {
    font-family: var(--font-ui);
    font-size: 11px;
    color: var(--text-primary);
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px 10px;
    font-weight: 700;
    letter-spacing: 0.6px;
    cursor: pointer;
    transition: border-color 0.12s, background 0.12s, box-shadow 0.12s;
    box-shadow: inset 0 -1px 0 0 color-mix(in srgb, var(--border) 60%, transparent);
  }
  .kbd-btn:hover {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 20%, transparent);
  }
  .kbd-btn.unbound {
    color: var(--text-muted);
    font-style: italic;
    font-weight: 400;
  }

  .row-actions {
    display: flex;
    gap: 4px;
  }
  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: 5px;
    background: none;
    border: 1px solid transparent;
    color: var(--text-muted);
    cursor: pointer;
    transition: background 0.12s, color 0.12s, border-color 0.12s;
  }
  .icon-btn:hover:not(:disabled) {
    background: var(--bg-surface);
    color: var(--text-primary);
    border-color: var(--border);
  }
  .icon-btn:disabled { opacity: 0.25; cursor: not-allowed; }

  /* Recording state */
  .recorder {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .recording-kbd {
    font-family: var(--font-ui);
    font-size: 12px;
    color: var(--accent);
    background: var(--bg-secondary);
    border: 1.5px solid var(--accent);
    border-radius: 6px;
    padding: 5px 12px;
    font-weight: 700;
    letter-spacing: 0.4px;
    min-width: 80px;
    text-align: center;
    animation: pulse 1.2s ease-in-out infinite;
  }
  @keyframes pulse {
    0%, 100% { box-shadow: 0 0 0 0 color-mix(in srgb, var(--accent) 30%, transparent); }
    50% { box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 15%, transparent); }
  }

  .conflict {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 10.5px;
    color: var(--warning);
    background: color-mix(in srgb, var(--warning) 12%, transparent);
    padding: 3px 8px;
    border-radius: 4px;
  }

  .recorder-actions {
    display: flex;
    gap: 4px;
  }
  .rec-btn {
    font-size: 11px;
    font-weight: 600;
    padding: 4px 10px;
    border-radius: 5px;
    border: 1px solid transparent;
    cursor: pointer;
    transition: background 0.12s, opacity 0.12s;
  }
  .rec-btn:disabled { opacity: 0.4; cursor: not-allowed; }
  .rec-btn.confirm {
    background: var(--accent);
    color: var(--bg-primary);
  }
  .rec-btn.confirm:hover:not(:disabled) { opacity: 0.85; }
  .rec-btn.cancel {
    background: var(--bg-surface);
    color: var(--text-secondary);
    border-color: var(--border);
  }
  .rec-btn.cancel:hover { background: var(--border); }
</style>
