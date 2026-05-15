/**
 * Agent plan store and utilities.
 *
 * The plan phase asks the model to produce a structured step list
 * before executing. Users can review, reorder, or remove steps.
 * Once approved, the agent executes steps sequentially.
 */
import { writable, get } from 'svelte/store';

// ── Types ──

export type PlanStepStatus = 'pending' | 'running' | 'done' | 'skipped' | 'failed';

export interface PlanStep {
  id: string;
  description: string;
  tool?: string;
  status: PlanStepStatus;
}

export interface AgentPlan {
  id: string;
  steps: PlanStep[];
  approved: boolean;
  createdAt: number;
}

// ── Store ──

export const currentPlan = writable<AgentPlan | null>(null);

// ── Plan creation ──

export function createPlan(steps: PlanStep[]): AgentPlan {
  const plan: AgentPlan = {
    id: `plan-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
    steps,
    approved: false,
    createdAt: Date.now(),
  };
  currentPlan.set(plan);
  return plan;
}

// ── Plan manipulation ──

export function approvePlan() {
  currentPlan.update(p => p ? { ...p, approved: true } : null);
}

export function rejectPlan() {
  currentPlan.set(null);
}

export function removeStep(stepId: string) {
  currentPlan.update(p => {
    if (!p) return null;
    return { ...p, steps: p.steps.filter(s => s.id !== stepId) };
  });
}

export function reorderSteps(stepIds: string[]) {
  currentPlan.update(p => {
    if (!p) return null;
    const byId = new Map(p.steps.map(s => [s.id, s]));
    const reordered = stepIds.map(id => byId.get(id)).filter(Boolean) as PlanStep[];
    return { ...p, steps: reordered };
  });
}

export function updateStepStatus(stepId: string, status: PlanStepStatus) {
  currentPlan.update(p => {
    if (!p) return null;
    return {
      ...p,
      steps: p.steps.map(s => s.id === stepId ? { ...s, status } : s),
    };
  });
}

// ── Plan parsing from model output ──

/**
 * Parse a numbered step list from the model's response.
 * Accepts formats like:
 *   1. Read the file src/main.ts
 *   2. Add error handling to the fetch call
 *   3. Run tests to verify
 *
 * Also handles:
 *   - Step one description
 *   - Step two description
 */
export function parsePlanSteps(text: string): PlanStep[] {
  const lines = text.split('\n').map(l => l.trim()).filter(Boolean);
  const steps: PlanStep[] = [];

  for (const line of lines) {
    // Match "1. description" or "1) description"
    const numbered = line.match(/^\d+[.)]\s+(.+)/);
    // Match "- description" or "* description"
    const bulleted = line.match(/^[-*]\s+(.+)/);

    const description = numbered?.[1] || bulleted?.[1];
    if (!description) continue;

    steps.push({
      id: `step-${steps.length}-${Math.random().toString(36).slice(2, 6)}`,
      description: description.trim(),
      tool: inferTool(description),
      status: 'pending',
    });
  }

  return steps;
}

/**
 * Infer which tool a step likely uses based on keywords.
 * This is a hint for the UI — the model decides the actual tool at execution time.
 */
function inferTool(description: string): string | undefined {
  const lower = description.toLowerCase();
  if (lower.includes('read') || lower.includes('look at') || lower.includes('examine') || lower.includes('check the file')) return 'read_file';
  if (lower.includes('edit') || lower.includes('modify') || lower.includes('change') || lower.includes('update') || lower.includes('add') || lower.includes('remove') || lower.includes('fix')) return 'edit_file';
  if (lower.includes('run') || lower.includes('test') || lower.includes('build') || lower.includes('execute') || lower.includes('install')) return 'run_command';
  if (lower.includes('search') || lower.includes('find') || lower.includes('grep') || lower.includes('look for')) return 'search_files';
  if (lower.includes('list') || lower.includes('directory') || lower.includes('folder')) return 'list_dir';
  return undefined;
}

// ── Plan prompt ──

export const PLAN_SYSTEM_PROMPT = `Before executing any changes, first create a numbered plan of steps you will take. Format your plan as a numbered list:

1. First step description
2. Second step description
3. Third step description

Keep each step concise (one sentence). Include what tool you'll use (read, edit, run, search). After listing the plan, stop and wait for approval. Do NOT execute any steps yet.`;
