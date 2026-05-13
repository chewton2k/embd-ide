<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { projectRoot } from '../../modules';
  import { get } from 'svelte/store';
  import { computeLayout, type LayoutNode, type LayoutEdge } from './layout';
  import { ZoomIn, ZoomOut, Maximize2, RefreshCw } from 'lucide-svelte';

  interface Props {
    filePath: string;
  }

  let { filePath }: Props = $props();

  let nodes = $state<LayoutNode[]>([]);
  let edges = $state<LayoutEdge[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  // Pan & zoom state
  let panX = $state(0);
  let panY = $state(0);
  let zoom = $state(1);
  let isPanning = $state(false);
  let panStart = { x: 0, y: 0, panX: 0, panY: 0 };

  // Drag node state
  let draggingNode = $state<string | null>(null);
  let dragStart = { x: 0, y: 0, nodeX: 0, nodeY: 0 };

  let containerEl: HTMLDivElement;

  const NODE_COLORS: Record<string, { bg: string; border: string; text: string }> = {
    target: { bg: '#1e3a5f', border: '#4a9eff', text: '#ffffff' },
    import: { bg: '#1a3d2e', border: '#4ec9b0', text: '#e0e0e0' },
    dependent: { bg: '#3d2e1a', border: '#e8a838', text: '#e0e0e0' },
    export: { bg: '#2d1f4e', border: '#b07aff', text: '#e0e0e0' },
    endpoint: { bg: '#4a1a1a', border: '#f14c4c', text: '#e0e0e0' },
    schema: { bg: '#1a3d3d', border: '#4ecdc4', text: '#e0e0e0' },
    database: { bg: '#2a1a3d', border: '#ff6bcb', text: '#e0e0e0' },
    external: { bg: '#2a2a2a', border: '#666666', text: '#999999' },
  };

  async function analyze() {
    loading = true;
    error = null;
    try {
      const graph = await invoke<any>('analyze_file_graph', {
        filePath,
        projectRoot: get(projectRoot),
      });
      const result = computeLayout(graph);
      nodes = result.nodes;
      edges = result.edges;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function fitToView() {
    if (nodes.length === 0) return;
    const minX = Math.min(...nodes.map(n => n.x - n.width / 2));
    const maxX = Math.max(...nodes.map(n => n.x + n.width / 2));
    const minY = Math.min(...nodes.map(n => n.y - n.height / 2));
    const maxY = Math.max(...nodes.map(n => n.y + n.height / 2));
    const graphW = maxX - minX + 100;
    const graphH = maxY - minY + 100;
    const containerW = containerEl?.clientWidth || 800;
    const containerH = containerEl?.clientHeight || 600;
    zoom = Math.min(containerW / graphW, containerH / graphH, 1.5);
    panX = -(minX + maxX) / 2 * zoom + containerW / 2;
    panY = -(minY + maxY) / 2 * zoom + containerH / 2;
  }

  function handleWheel(e: WheelEvent) {
    e.preventDefault();
    const factor = e.deltaY > 0 ? 0.9 : 1.1;
    const newZoom = Math.max(0.2, Math.min(4, zoom * factor));
    // Zoom toward cursor
    const rect = containerEl.getBoundingClientRect();
    const cx = e.clientX - rect.left;
    const cy = e.clientY - rect.top;
    panX = cx - (cx - panX) * (newZoom / zoom);
    panY = cy - (cy - panY) * (newZoom / zoom);
    zoom = newZoom;
  }

  function handleMouseDown(e: MouseEvent) {
    if (e.button !== 0) return;
    // Check if clicking on a node
    const target = e.target as HTMLElement;
    if (target.closest('.diagram-node')) return;
    isPanning = true;
    panStart = { x: e.clientX, y: e.clientY, panX, panY };
  }

  function handleMouseMove(e: MouseEvent) {
    if (isPanning) {
      panX = panStart.panX + (e.clientX - panStart.x);
      panY = panStart.panY + (e.clientY - panStart.y);
    }
    if (draggingNode) {
      const dx = (e.clientX - dragStart.x) / zoom;
      const dy = (e.clientY - dragStart.y) / zoom;
      const node = nodes.find(n => n.id === draggingNode);
      if (node) {
        node.x = dragStart.nodeX + dx;
        node.y = dragStart.nodeY + dy;
        nodes = [...nodes]; // trigger reactivity
      }
    }
  }

  function handleMouseUp() {
    isPanning = false;
    draggingNode = null;
  }

  function startNodeDrag(e: MouseEvent, nodeId: string) {
    e.stopPropagation();
    const node = nodes.find(n => n.id === nodeId);
    if (!node) return;
    draggingNode = nodeId;
    dragStart = { x: e.clientX, y: e.clientY, nodeX: node.x, nodeY: node.y };
  }

  function getEdgePath(edge: LayoutEdge): string {
    const from = nodes.find(n => n.id === edge.from);
    const to = nodes.find(n => n.id === edge.to);
    if (!from || !to) return '';

    const x1 = from.x;
    const y1 = from.y;
    const x2 = to.x;
    const y2 = to.y;

    // Cubic bezier with control points offset toward the midpoint
    const dx = x2 - x1;
    const dy = y2 - y1;
    const cx1 = x1 + dx * 0.4;
    const cy1 = y1;
    const cx2 = x2 - dx * 0.4;
    const cy2 = y2;

    return `M ${x1} ${y1} C ${cx1} ${cy1}, ${cx2} ${cy2}, ${x2} ${y2}`;
  }

  function getEdgeMidpoint(edge: LayoutEdge): { x: number; y: number } {
    const from = nodes.find(n => n.id === edge.from);
    const to = nodes.find(n => n.id === edge.to);
    if (!from || !to) return { x: 0, y: 0 };
    return { x: (from.x + to.x) / 2, y: (from.y + to.y) / 2 };
  }

  $effect(() => {
    filePath; // track
    analyze().then(() => {
      requestAnimationFrame(fitToView);
    });
  });
</script>

<div class="diagram-container" bind:this={containerEl}>
  <div class="diagram-toolbar">
    <button onclick={() => { zoom = Math.min(4, zoom * 1.2); }} title="Zoom in">
      <ZoomIn size={14} />
    </button>
    <button onclick={() => { zoom = Math.max(0.2, zoom * 0.8); }} title="Zoom out">
      <ZoomOut size={14} />
    </button>
    <button onclick={fitToView} title="Fit to view">
      <Maximize2 size={14} />
    </button>
    <button onclick={() => analyze().then(() => requestAnimationFrame(fitToView))} title="Refresh">
      <RefreshCw size={14} />
    </button>
    <span class="zoom-label">{Math.round(zoom * 100)}%</span>
  </div>

  {#if loading}
    <div class="diagram-status">Analyzing file...</div>
  {:else if error}
    <div class="diagram-status error">{error}</div>
  {:else}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      class="diagram-canvas"
      onmousedown={handleMouseDown}
      onmousemove={handleMouseMove}
      onmouseup={handleMouseUp}
      onmouseleave={handleMouseUp}
      onwheel={handleWheel}
      role="application"
    >
      <svg
        class="diagram-svg"
        style="transform: translate({panX}px, {panY}px) scale({zoom})"
      >
        <defs>
          <marker id="arrowhead" markerWidth="8" markerHeight="6" refX="8" refY="3" orient="auto">
            <polygon points="0 0, 8 3, 0 6" fill="#666" />
          </marker>
        </defs>

        <!-- Edges -->
        {#each edges as edge (edge.id)}
          <path
            d={getEdgePath(edge)}
            fill="none"
            stroke="#555"
            stroke-width="1.5"
            stroke-dasharray={edge.dashed ? '5,3' : 'none'}
            marker-end="url(#arrowhead)"
            opacity="0.7"
          />
          {#if edge.label}
            {@const midpoint = getEdgeMidpoint(edge)}
            <text x={midpoint.x} y={midpoint.y - 6} text-anchor="middle" fill="#888" font-size="10">{edge.label}</text>
          {/if}
        {/each}
      </svg>

      <!-- Nodes rendered as HTML for better text handling -->
      {#each nodes as node (node.id)}
        {@const colors = NODE_COLORS[node.type] || NODE_COLORS.external}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="diagram-node"
          class:active={draggingNode === node.id}
          style="
            left: {panX + (node.x - node.width / 2) * zoom}px;
            top: {panY + (node.y - node.height / 2) * zoom}px;
            width: {node.width * zoom}px;
            height: {node.height * zoom}px;
            background: {colors.bg};
            border-color: {colors.border};
            color: {colors.text};
            font-size: {Math.max(9, 12 * zoom)}px;
          "
          onmousedown={(e) => startNodeDrag(e, node.id)}
          title={node.sublabel || node.label}
        >
          <span class="node-label">{node.label}</span>
          {#if node.sublabel && node.type !== 'target'}
            <span class="node-sublabel" style="font-size: {Math.max(7, 9 * zoom)}px">{node.sublabel}</span>
          {/if}
          {#if node.details && zoom > 0.6}
            <div class="node-details" style="font-size: {Math.max(7, 9 * zoom)}px">
              {#each node.details as detail}
                <span class="node-detail-line">{detail}</span>
              {/each}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .diagram-container {
    width: 100%;
    height: 100%;
    position: relative;
    overflow: hidden;
    background: var(--bg-primary);
  }

  .diagram-toolbar {
    position: absolute;
    top: 8px;
    right: 8px;
    z-index: 10;
    display: flex;
    align-items: center;
    gap: 4px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px 6px;
  }

  .diagram-toolbar button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border-radius: 4px;
    color: var(--text-secondary);
    cursor: pointer;
    transition: background 0.1s;
  }

  .diagram-toolbar button:hover {
    background: var(--bg-surface);
    color: var(--text-primary);
  }

  .zoom-label {
    font-size: 11px;
    color: var(--text-muted);
    margin-left: 4px;
    min-width: 36px;
    text-align: center;
  }

  .diagram-canvas {
    width: 100%;
    height: 100%;
    cursor: grab;
    position: relative;
    overflow: hidden;
  }

  .diagram-canvas:active {
    cursor: grabbing;
  }

  .diagram-svg {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    overflow: visible;
    transform-origin: 0 0;
    pointer-events: none;
  }

  .diagram-node {
    position: absolute;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    border: 1.5px solid;
    border-radius: 6px;
    padding: 4px 8px;
    cursor: grab;
    user-select: none;
    transition: box-shadow 0.15s;
    overflow: hidden;
    text-align: center;
    box-sizing: border-box;
  }

  .diagram-node:hover {
    box-shadow: 0 0 12px rgba(74, 158, 255, 0.3);
    z-index: 5;
  }

  .diagram-node.active {
    cursor: grabbing;
    box-shadow: 0 0 16px rgba(74, 158, 255, 0.5);
    z-index: 10;
  }

  .node-label {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
    font-weight: 500;
    line-height: 1.2;
  }

  .node-sublabel {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
    opacity: 0.6;
    line-height: 1.2;
  }

  .node-details {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    width: 100%;
    padding-top: 2px;
    border-top: 1px solid rgba(255,255,255,0.1);
    margin-top: 2px;
    overflow: hidden;
  }

  .node-detail-line {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
    opacity: 0.7;
    line-height: 1.4;
    font-family: var(--font-mono, monospace);
  }

  .diagram-status {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    color: var(--text-muted);
    font-size: 13px;
  }

  .diagram-status.error {
    color: var(--text-error, #f14c4c);
  }
</style>
