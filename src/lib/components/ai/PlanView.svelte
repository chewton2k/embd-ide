<!--
  PlanView.svelte — Checklist UI for the agent plan.
  Shows steps with status indicators, approve/reject buttons.
-->
<script lang="ts">
  import { currentPlan, approvePlan, rejectPlan, removeStep, type PlanStepStatus } from '../../modules/ai/agentPlan';
  import { executePlan } from '../../modules/ai/agentLoop';

  const STATUS_ICONS: Record<PlanStepStatus, string> = {
    pending: '○',
    running: '◉',
    done: '✓',
    skipped: '–',
    failed: '✗',
  };

  function handleApprove() {
    approvePlan();
    executePlan();
  }
</script>

{#if $currentPlan}
  <div class="plan-view">
    <div class="plan-header">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="14" height="14">
        <path d="M9 11l3 3L22 4" /><path d="M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11" />
      </svg>
      <span class="plan-title">Plan</span>
      <span class="plan-count">{$currentPlan.steps.length} steps</span>
    </div>

    <ol class="plan-steps">
      {#each $currentPlan.steps as step (step.id)}
        <li class="step" class:running={step.status === 'running'} class:done={step.status === 'done'} class:failed={step.status === 'failed'}>
          <span class="step-icon" class:running={step.status === 'running'} class:done={step.status === 'done'} class:failed={step.status === 'failed'}>
            {STATUS_ICONS[step.status]}
          </span>
          <span class="step-text">{step.description}</span>
          {#if step.tool && step.status === 'pending'}
            <span class="step-tool">{step.tool}</span>
          {/if}
          {#if !$currentPlan.approved && step.status === 'pending'}
            <button class="step-remove" onclick={() => removeStep(step.id)} aria-label="Remove step">×</button>
          {/if}
        </li>
      {/each}
    </ol>

    {#if !$currentPlan.approved}
      <div class="plan-actions">
        <button class="btn btn-approve" onclick={handleApprove}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" width="12" height="12">
            <path d="M20 6L9 17l-5-5" />
          </svg>
          Execute Plan
        </button>
        <button class="btn btn-reject" onclick={rejectPlan}>Cancel</button>
      </div>
    {/if}
  </div>
{/if}

<style>
  .plan-view {
    background: var(--bg-tertiary, #1a1a2e);
    border: 1px solid var(--border, #333);
    border-radius: 8px;
    padding: 12px;
    margin: 8px 0;
  }

  .plan-header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 10px;
    color: var(--text-muted, #888);
  }

  .plan-title {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-primary, #e0e0e0);
  }

  .plan-count {
    font-size: 11px;
    margin-left: auto;
  }

  .plan-steps {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .step {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-radius: 6px;
    font-size: 12px;
    color: var(--text-secondary, #ccc);
    transition: background 0.12s;
  }

  .step.running { background: color-mix(in srgb, var(--accent, #4a9eff) 10%, transparent); }
  .step.done { opacity: 0.7; }
  .step.failed { opacity: 0.7; color: var(--error, #f14c4c); }

  .step-icon {
    flex-shrink: 0;
    width: 16px;
    text-align: center;
    font-size: 13px;
    color: var(--text-muted, #888);
  }
  .step-icon.running { color: var(--accent, #4a9eff); animation: pulse 1s infinite; }
  .step-icon.done { color: var(--success, #4ec9b0); }
  .step-icon.failed { color: var(--error, #f14c4c); }

  .step-text { flex: 1; min-width: 0; }

  .step-tool {
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 4px;
    background: var(--bg-surface, #2a2a3a);
    color: var(--text-muted, #888);
  }

  .step-remove {
    flex-shrink: 0;
    width: 18px;
    height: 18px;
    display: flex;
    align-items: center;
    justify-content: center;
    border: none;
    background: none;
    color: var(--text-muted, #888);
    cursor: pointer;
    border-radius: 4px;
    font-size: 14px;
    opacity: 0;
    transition: opacity 0.12s, background 0.12s;
  }
  .step:hover .step-remove { opacity: 1; }
  .step-remove:hover { background: var(--bg-surface, #2a2a3a); color: var(--error, #f14c4c); }

  .plan-actions {
    display: flex;
    gap: 8px;
    margin-top: 10px;
    padding-top: 10px;
    border-top: 1px solid var(--border, #333);
  }

  .btn {
    padding: 5px 12px;
    border-radius: 6px;
    font-size: 11.5px;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid transparent;
    display: flex;
    align-items: center;
    gap: 5px;
    transition: background 0.12s, opacity 0.12s;
  }

  .btn-approve {
    background: var(--accent, #4a9eff);
    color: #fff;
  }
  .btn-approve:hover { opacity: 0.85; }

  .btn-reject {
    background: var(--bg-surface, #2a2a3a);
    color: var(--text-secondary, #ccc);
    border-color: var(--border, #333);
  }
  .btn-reject:hover { background: var(--border, #333); }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }
</style>
