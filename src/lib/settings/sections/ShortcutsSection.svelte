<script lang="ts">
  import SectionHeader from '../components/SectionHeader.svelte';
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

<div class="root" data-setting="shortcuts">
  <SectionHeader
    title="Shortcuts"
    description="Built-in keyboard shortcuts. A configurable recorder is on the roadmap."
  />

  {#each GROUPS as group}
    <div class="card">
      <div class="card-head">{group.name}</div>
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
</div>

<style>
  .root { display: flex; flex-direction: column; gap: 16px; }
  .card {
    background: color-mix(in srgb, var(--bg-tertiary) 60%, transparent);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 14px 16px;
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
    display: flex; align-items: center; justify-content: space-between;
    padding: 10px 0;
  }
  .row + .row { border-top: 1px solid color-mix(in srgb, var(--border) 60%, transparent); }
  .label { font-size: 13px; color: var(--text-primary); font-weight: 500; }
  kbd {
    font-family: var(--font-ui);
    font-size: 12px;
    color: var(--text-primary);
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px 10px;
    font-weight: 700;
    letter-spacing: 0.5px;
  }
</style>
