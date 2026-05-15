import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
  parsePlanSteps,
  createPlan,
  approvePlan,
  rejectPlan,
  removeStep,
  reorderSteps,
  updateStepStatus,
  currentPlan,
  type PlanStep,
} from '$lib/modules/ai/agentPlan';

describe('agentPlan', () => {
  beforeEach(() => {
    currentPlan.set(null);
  });

  describe('parsePlanSteps', () => {
    it('parses numbered list with dots', () => {
      const text = `1. Read the file src/main.ts
2. Add error handling
3. Run tests`;
      const steps = parsePlanSteps(text);
      expect(steps).toHaveLength(3);
      expect(steps[0].description).toBe('Read the file src/main.ts');
      expect(steps[1].description).toBe('Add error handling');
      expect(steps[2].description).toBe('Run tests');
    });

    it('parses numbered list with parentheses', () => {
      const text = `1) First step
2) Second step`;
      const steps = parsePlanSteps(text);
      expect(steps).toHaveLength(2);
      expect(steps[0].description).toBe('First step');
    });

    it('parses bulleted list with dashes', () => {
      const text = `- Read the config
- Edit the handler
- Test the changes`;
      const steps = parsePlanSteps(text);
      expect(steps).toHaveLength(3);
    });

    it('parses bulleted list with asterisks', () => {
      const text = `* Step one
* Step two`;
      const steps = parsePlanSteps(text);
      expect(steps).toHaveLength(2);
    });

    it('ignores non-list lines', () => {
      const text = `Here is my plan:

1. Read the file
2. Edit the function

Let me know if this looks good.`;
      const steps = parsePlanSteps(text);
      expect(steps).toHaveLength(2);
    });

    it('returns empty array for no steps', () => {
      expect(parsePlanSteps('Just some text without steps')).toHaveLength(0);
    });

    it('handles empty input', () => {
      expect(parsePlanSteps('')).toHaveLength(0);
    });

    it('all steps start as pending', () => {
      const steps = parsePlanSteps('1. Do something\n2. Do another thing');
      for (const step of steps) {
        expect(step.status).toBe('pending');
      }
    });

    it('assigns unique IDs to each step', () => {
      const steps = parsePlanSteps('1. A\n2. B\n3. C');
      const ids = new Set(steps.map(s => s.id));
      expect(ids.size).toBe(3);
    });

    it('infers read_file tool from keywords', () => {
      const steps = parsePlanSteps('1. Read the configuration file');
      expect(steps[0].tool).toBe('read_file');
    });

    it('infers edit_file tool from keywords', () => {
      const steps = parsePlanSteps('1. Edit the handler to add validation');
      expect(steps[0].tool).toBe('edit_file');
    });

    it('infers run_command tool from keywords', () => {
      const steps = parsePlanSteps('1. Run the test suite');
      expect(steps[0].tool).toBe('run_command');
    });

    it('infers search_files tool from keywords', () => {
      const steps = parsePlanSteps('1. Search for usages of the function');
      expect(steps[0].tool).toBe('search_files');
    });
  });

  describe('plan store operations', () => {
    it('createPlan sets the current plan', () => {
      const steps: PlanStep[] = [
        { id: 's1', description: 'Step 1', status: 'pending' },
        { id: 's2', description: 'Step 2', status: 'pending' },
      ];
      createPlan(steps);
      const plan = get(currentPlan);
      expect(plan).not.toBeNull();
      expect(plan!.steps).toHaveLength(2);
      expect(plan!.approved).toBe(false);
    });

    it('approvePlan sets approved to true', () => {
      createPlan([{ id: 's1', description: 'X', status: 'pending' }]);
      approvePlan();
      expect(get(currentPlan)!.approved).toBe(true);
    });

    it('rejectPlan clears the plan', () => {
      createPlan([{ id: 's1', description: 'X', status: 'pending' }]);
      rejectPlan();
      expect(get(currentPlan)).toBeNull();
    });

    it('removeStep removes a step by ID', () => {
      createPlan([
        { id: 's1', description: 'A', status: 'pending' },
        { id: 's2', description: 'B', status: 'pending' },
        { id: 's3', description: 'C', status: 'pending' },
      ]);
      removeStep('s2');
      const plan = get(currentPlan)!;
      expect(plan.steps).toHaveLength(2);
      expect(plan.steps.map(s => s.id)).toEqual(['s1', 's3']);
    });

    it('reorderSteps reorders by ID list', () => {
      createPlan([
        { id: 's1', description: 'A', status: 'pending' },
        { id: 's2', description: 'B', status: 'pending' },
        { id: 's3', description: 'C', status: 'pending' },
      ]);
      reorderSteps(['s3', 's1', 's2']);
      const plan = get(currentPlan)!;
      expect(plan.steps.map(s => s.id)).toEqual(['s3', 's1', 's2']);
    });

    it('updateStepStatus changes a step status', () => {
      createPlan([
        { id: 's1', description: 'A', status: 'pending' },
        { id: 's2', description: 'B', status: 'pending' },
      ]);
      updateStepStatus('s1', 'running');
      expect(get(currentPlan)!.steps[0].status).toBe('running');
      updateStepStatus('s1', 'done');
      expect(get(currentPlan)!.steps[0].status).toBe('done');
    });

    it('operations on null plan are safe', () => {
      // Should not throw
      approvePlan();
      removeStep('nonexistent');
      reorderSteps(['a', 'b']);
      updateStepStatus('x', 'done');
      expect(get(currentPlan)).toBeNull();
    });
  });
});
