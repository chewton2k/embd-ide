<script lang="ts">
  // Placeholder until a custom-agents subsystem lands.
</script>

<div class="placeholder">
  <div class="icon">
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" width="32" height="32">
      <circle cx="9" cy="8" r="3" />
      <path d="M3 21v-1a6 6 0 0 1 6-6 6 6 0 0 1 6 6v1" />
      <circle cx="17" cy="6" r="2" />
      <path d="M14 12a4 4 0 0 1 4-4 4 4 0 0 1 4 4v0" />
    </svg>
  </div>
  <h3>Agents & snippets</h3>
  <p>
    Build personalized AI agents and reusable instruction snippets here.
    This area is reserved for an upcoming release.
  </p>
  <ul class="roadmap">
    <li>Built-in agents (coder, reviewer, architect)</li>
    <li>Custom agents with icon, persona, and rules</li>
    <li>Reusable snippets referenced with <code>#handle</code> syntax</li>
  </ul>
</div>

<style>
  .placeholder {
    max-width: 480px;
    margin: 32px auto 0;
    padding: 28px;
    border: 1px dashed var(--border);
    border-radius: 10px;
    text-align: center;
    color: var(--text-secondary);
  }
  .icon { color: var(--accent); margin-bottom: 12px; display: flex; justify-content: center; }
  h3 {
    font-size: 14px; font-weight: 600;
    color: var(--text-primary);
    margin: 0 0 8px;
  }
  p { font-size: 12px; line-height: 1.6; margin: 0 0 18px; }
  .roadmap {
    text-align: left;
    font-size: 12px;
    color: var(--text-muted);
    margin: 0; padding: 0 0 0 20px;
    line-height: 1.8;
  }
  code {
    background: var(--bg-surface);
    padding: 1px 4px;
    border-radius: 3px;
    font-size: 11px;
  }
</style>
