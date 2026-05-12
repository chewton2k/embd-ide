<script lang="ts">
  // Built-in shortcuts list — read-only for now. A recorder/editor is
  // planned but requires a centralized shortcut registry first.
  const GROUPS: { name: string; items: { label: string; keys: string }[] }[] = [
    {
      name: 'File',
      items: [
        { label: 'Open file search',    keys: '⌘P' },
        { label: 'Open folder',         keys: '⌘O' },
        { label: 'Save file',           keys: '⌘S' },
        { label: 'Close tab',           keys: '⌘W' },
      ],
    },
    {
      name: 'Tabs',
      items: [
        { label: 'Next tab',            keys: '⌃Tab' },
        { label: 'Previous tab',        keys: '⌃⇧Tab' },
        { label: 'Go to tab 1–9',       keys: '⌘1 … ⌘9' },
      ],
    },
    {
      name: 'View',
      items: [
        { label: 'Toggle sidebar',      keys: '⌘B' },
        { label: 'Toggle terminal',     keys: '⌃`' },
        { label: 'Toggle chat panel',   keys: '⌘⇧A' },
        { label: 'Toggle source control', keys: '⌘⇧G' },
        { label: 'Open settings',       keys: '⌘,' },
      ],
    },
    {
      name: 'Editor',
      items: [
        { label: 'Find in file',        keys: '⌘F' },
        { label: 'Replace in file',     keys: '⌘⌥F' },
        { label: 'Go to line',          keys: '⌃G' },
      ],
    },
  ];
</script>

<div class="banner">
  <strong>Read-only for now.</strong> A keybinding recorder is on the roadmap.
</div>

{#each GROUPS as group}
  <div class="section">
    <h3>{group.name}</h3>
    <div class="list">
      {#each group.items as item}
        <div class="row">
          <span class="label">{item.label}</span>
          <kbd>{item.keys}</kbd>
        </div>
      {/each}
    </div>
  </div>
{/each}

<style>
  .banner {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent) 30%, transparent);
    color: var(--text-primary);
    padding: 10px 14px;
    border-radius: 6px;
    font-size: 12px;
    margin-bottom: 24px;
  }
  .section { margin-bottom: 24px; }
  .section h3 {
    font-size: 11px; font-weight: 600;
    color: var(--text-secondary);
    margin: 0 0 8px;
    text-transform: uppercase;
    letter-spacing: 0.6px;
  }
  .list { display: flex; flex-direction: column; gap: 2px; }
  .row {
    display: flex; align-items: center; justify-content: space-between;
    padding: 6px 8px; border-radius: 4px;
  }
  .row:hover { background: var(--bg-surface); }
  .label { font-size: 13px; color: var(--text-primary); }
  kbd {
    font-family: var(--font-mono, monospace);
    font-size: 11px;
    color: var(--text-secondary);
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 2px 8px;
  }
</style>
