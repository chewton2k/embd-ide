export interface AiDiffResolveDetail {
  id: string;
  action: 'approve' | 'reject';
}

export function bindAiDiffResolve(
  container: HTMLElement,
  onResolve: (detail: AiDiffResolveDetail) => void,
): () => void {
  const handler = (event: Event) => {
    const customEvent = event as CustomEvent<AiDiffResolveDetail>;
    onResolve(customEvent.detail);
  };

  container.addEventListener('ai-diff-resolve', handler as EventListener);
  return () => {
    container.removeEventListener('ai-diff-resolve', handler as EventListener);
  };
}
