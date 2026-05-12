<script lang="ts">
  import { onMount } from 'svelte';
  import { fade, fly, scale } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import { FolderOpen, MessageSquare, Trash2 } from 'lucide-svelte';
  import {
    listConversations, shortProjectName, formatRelativeTime, formatBytes,
    type ProjectInfo, type ConversationSummary,
  } from '../../modules/knowledge';

  // ── Props ────────────────────────────────────────────────────

  interface Props {
    projects: ProjectInfo[];
    onOpenConversation: (projectRoot: string, conv: ConversationSummary) => void;
    onDeleteProject: (projectRoot: string) => void;
  }
  let { projects, onOpenConversation, onDeleteProject }: Props = $props();

  // ── State ────────────────────────────────────────────────────

  /** Lazy-loaded conversation lists, keyed by project_root. Projects with 0
   *  conversations are skipped (no point in fetching). */
  let conversationsByProject = $state<Record<string, ConversationSummary[]>>({});
  let loading = $state(true);
  let error = $state<string | null>(null);
  let hoverHub = $state<string | null>(null);
  let hoverConv = $state<string | null>(null);
  /** Set after first successful paint so entry animations only play once. */
  let mounted = $state(false);

  onMount(async () => {
    try {
      const pairs = await Promise.all(
        projects
          .filter(p => p.conversation_count > 0 && p.project_root !== '(unknown)')
          .map(async (p): Promise<[string, ConversationSummary[]]> => {
            try {
              const list = await listConversations(p.project_root);
              return [p.project_root, list];
            } catch {
              return [p.project_root, []];
            }
          })
      );
      const map: Record<string, ConversationSummary[]> = {};
      for (const [root, list] of pairs) map[root] = list;
      conversationsByProject = map;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
      // Delay one frame so the SVG has laid out before the entry transition
      // kicks off — avoids a flash of unscaled content.
      requestAnimationFrame(() => (mounted = true));
    }
  });

  // ── Layout ───────────────────────────────────────────────────

  const CLUSTER_W = 320;
  const CLUSTER_H = 280;
  const HUB_R = 40;
  const DOT_R = 7;
  const ORBIT_R = HUB_R + 42;
  /** Max conversations drawn as dots per project before we truncate. */
  const MAX_DOTS = 16;

  function dotPosition(index: number, total: number): { x: number; y: number } {
    const n = Math.max(3, total);
    const angle = (index / n) * Math.PI * 2 - Math.PI / 2;
    return { x: Math.cos(angle) * ORBIT_R, y: Math.sin(angle) * ORBIT_R };
  }

  const columnCount = $derived(() => {
    const n = projects.length;
    if (n <= 1) return 1;
    if (n <= 4) return 2;
    return 3;
  });
  const rowCount = $derived(() => Math.ceil(projects.length / columnCount()));
  const svgW = $derived(() => columnCount() * CLUSTER_W);
  const svgH = $derived(() => rowCount() * CLUSTER_H);

  function clusterXY(i: number): { cx: number; cy: number } {
    const cols = columnCount();
    const col = i % cols;
    const row = Math.floor(i / cols);
    return {
      cx: col * CLUSTER_W + CLUSTER_W / 2,
      cy: row * CLUSTER_H + CLUSTER_H / 2,
    };
  }

  function focusProjectRow(root: string) {
    const row = document.querySelector<HTMLElement>(`[data-project-row="${CSS.escape(root)}"]`);
    if (!row) return;
    row.scrollIntoView({ behavior: 'smooth', block: 'center' });
    row.classList.remove('highlight');
    // Force reflow so the class re-addition restarts the animation.
    void row.offsetWidth;
    row.classList.add('highlight');
    // Remove after animation ends so it can be re-triggered.
    window.setTimeout(() => row.classList.remove('highlight'), 1300);
  }

  function handleHubKey(e: KeyboardEvent, root: string) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      focusProjectRow(root);
    }
  }

  function handleDotKey(e: KeyboardEvent, root: string, conv: ConversationSummary) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      onOpenConversation(root, conv);
    }
  }
</script>

{#if projects.length === 0}
  <div class="empty" in:fade={{ duration: 180 }}>
    <div class="empty-icon"><FolderOpen size={28} /></div>
    <p class="empty-title">No knowledge indexed yet</p>
    <p class="empty-hint">Open a project and chat with the AI — conversations will appear here.</p>
  </div>
{:else if loading}
  <div class="status" in:fade={{ duration: 180 }}>
    <div class="skeleton">
      <span></span><span></span><span></span>
    </div>
    <span>Loading conversations…</span>
  </div>
{:else if error}
  <div class="status error" in:fade={{ duration: 180 }}>{error}</div>
{:else}
  <div class="graph-wrap" in:fade={{ duration: 200 }}>
    <svg
      class="graph"
      viewBox="0 0 {svgW()} {svgH()}"
      preserveAspectRatio="xMidYMid meet"
      role="img"
      aria-label="Knowledge graph: projects as hubs with their conversations orbiting"
    >
      <defs>
        <!-- Soft dot grid -->
        <pattern id="kg-dots" width="20" height="20" patternUnits="userSpaceOnUse">
          <circle cx="10" cy="10" r="0.8" fill="var(--text-muted)" opacity="0.18" />
        </pattern>

        <!-- Hub gradient: accent core fading to surface at the edge. -->
        <radialGradient id="hub-grad" cx="35%" cy="30%" r="80%">
          <stop offset="0%"  stop-color="color-mix(in srgb, var(--accent) 35%, var(--bg-surface))" />
          <stop offset="70%" stop-color="var(--bg-surface)" />
          <stop offset="100%" stop-color="color-mix(in srgb, var(--bg-surface) 60%, transparent)" />
        </radialGradient>
        <radialGradient id="hub-grad-hover" cx="35%" cy="30%" r="80%">
          <stop offset="0%"  stop-color="color-mix(in srgb, var(--accent) 55%, var(--bg-surface))" />
          <stop offset="70%" stop-color="color-mix(in srgb, var(--accent) 12%, var(--bg-surface))" />
          <stop offset="100%" stop-color="color-mix(in srgb, var(--bg-surface) 60%, transparent)" />
        </radialGradient>

        <!-- Spoke gradient: fades from hub (strong) to dot (weak). -->
        <linearGradient id="spoke-grad" x1="0%" y1="0%" x2="100%" y2="0%">
          <stop offset="0%"  stop-color="var(--accent)" stop-opacity="0.55" />
          <stop offset="100%" stop-color="var(--accent)" stop-opacity="0.08" />
        </linearGradient>

        <!-- Subtle drop shadow used on hovered hubs. -->
        <filter id="hub-glow" x="-50%" y="-50%" width="200%" height="200%">
          <feGaussianBlur stdDeviation="6" />
          <feComponentTransfer>
            <feFuncA type="linear" slope="0.7" />
          </feComponentTransfer>
          <feMerge>
            <feMergeNode /><feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>
      </defs>

      <rect x="0" y="0" width={svgW()} height={svgH()} fill="url(#kg-dots)" />

      {#each projects as project, i (project.db_hash)}
        {@const center = clusterXY(i)}
        {@const convs = conversationsByProject[project.project_root] ?? []}
        {@const shown = convs.slice(0, MAX_DOTS)}
        {@const clipped = Math.max(0, convs.length - shown.length)}
        {@const hubHovered = hoverHub === project.project_root}

        <!--
          Outer <g> holds the static positional translate (SVG attribute);
          inner <g> carries the CSS-driven entry transform (scale). Keeping
          them on separate elements avoids the CSS-transform-replaces-
          attribute gotcha that would otherwise collapse all clusters to
          the SVG origin during the entry animation.
        -->
        <g transform="translate({center.cx}, {center.cy})">
          <g
            class="cluster"
            class:entered={mounted}
            style="--enter-delay: {i * 65}ms"
          >
          <!-- Rotating orbit ring (CSS animation). -->
          {#if shown.length > 0}
            <circle class="orbit" r={ORBIT_R} />
          {/if}

          <!-- Animated spokes (gradient, dashed), drawn first so dots sit on top. -->
          {#each shown as conv, di (conv.id)}
            {@const pos = dotPosition(di, shown.length)}
            {@const lineHover = hoverConv === conv.id || hoverHub === project.project_root}
            <line
              x1="0" y1="0" x2={pos.x} y2={pos.y}
              class="spoke"
              class:lit={lineHover}
              stroke="url(#spoke-grad)"
            />
          {/each}

          <!-- Hub (project): clickable as a navigation target — clicking
               scrolls the detail row below into view and briefly highlights
               it. Deleting is intentionally NOT wired to the click handler
               because that made accidental data loss too easy; the explicit
               "Delete" button lives in the detail row. -->
          <g
            class="hub"
            class:active={hubHovered}
            role="button"
            tabindex="0"
            aria-label="Project {shortProjectName(project.project_root)}. Click to focus details; a Delete button is available there."
            onmouseenter={() => hoverHub = project.project_root}
            onmouseleave={() => hoverHub = null}
            onfocus={() => hoverHub = project.project_root}
            onblur={() => hoverHub = null}
            onclick={() => focusProjectRow(project.project_root)}
            onkeydown={(e) => handleHubKey(e, project.project_root)}
          >
            <!-- Outer stroke ring (pulses on hover). -->
            <circle r={HUB_R + 4} class="hub-ring" />
            <!-- Hub body with gradient + optional glow filter on hover. -->
            <circle
              r={HUB_R}
              class="hub-bg"
              fill="url({hubHovered ? '#hub-grad-hover' : '#hub-grad'})"
              filter={hubHovered ? 'url(#hub-glow)' : ''}
            />
            <!-- Two text rows, tightly set. -->
            <text class="hub-count" y="-4" text-anchor="middle">
              {project.conversation_count}
            </text>
            <text class="hub-title" y="14" text-anchor="middle">
              {shortProjectName(project.project_root, 1)}
            </text>
          </g>

          <!-- Conversation dots. -->
          {#each shown as conv, di (conv.id)}
            {@const pos = dotPosition(di, shown.length)}
            {@const dotHovered = hoverConv === conv.id}
            <g
              class="dot-group"
              class:active={dotHovered}
              transform="translate({pos.x}, {pos.y})"
              role="button"
              tabindex="0"
              aria-label="Conversation: {conv.title}"
              onmouseenter={() => hoverConv = conv.id}
              onmouseleave={() => hoverConv = null}
              onfocus={() => hoverConv = conv.id}
              onblur={() => hoverConv = null}
              onclick={() => onOpenConversation(project.project_root, conv)}
              onkeydown={(e) => handleDotKey(e, project.project_root, conv)}
            >
              <!-- Glow ring that fades in on hover. -->
              <circle r={DOT_R + 6} class="dot-glow" />
              <circle r={DOT_R} class="dot" />
              {#if dotHovered}
                <g class="tooltip" transform="translate(0, {-DOT_R - 10})">
                  <rect
                    x="-88" y="-24" width="176" height="22" rx="6"
                    class="tooltip-bg"
                  />
                  <text y="-9" text-anchor="middle" class="tooltip-text">
                    {conv.title.slice(0, 40)}{conv.title.length > 40 ? '…' : ''}
                  </text>
                </g>
              {/if}
            </g>
          {/each}

          <!-- "+N more" clip indicator. -->
          {#if clipped > 0}
            <text y={ORBIT_R + 24} text-anchor="middle" class="more-label">
              +{clipped} more
            </text>
          {/if}
          </g>
        </g>
      {/each}
    </svg>
  </div>

  <!-- Per-project details below the graph. Stagger-animated on mount. -->
  <div class="details">
    {#each projects as project, i (project.db_hash)}
      <div
        class="detail-row"
        data-project-row={project.project_root}
        in:fly={{ y: 6, duration: 220, delay: 80 + i * 45, easing: cubicOut }}
      >
        <div class="detail-text">
          <div class="detail-title" title={project.project_root}>
            {shortProjectName(project.project_root)}
          </div>
          <div class="detail-meta">
            <MessageSquare size={10} />
            <span>{project.conversation_count} chat{project.conversation_count === 1 ? '' : 's'}</span>
            <span class="dot-sep">·</span>
            <span>{project.file_count} files</span>
            <span class="dot-sep">·</span>
            <span>{formatBytes(project.db_size_bytes)}</span>
            <span class="dot-sep">·</span>
            <span>updated {formatRelativeTime(project.last_updated)}</span>
          </div>
        </div>
        <button
          type="button"
          class="del-btn"
          onclick={() => onDeleteProject(project.project_root)}
          title="Delete this project's knowledge"
          aria-label="Delete {shortProjectName(project.project_root)}"
        >
          <Trash2 size={11} /> Delete
        </button>
      </div>
    {/each}
  </div>
{/if}

<style>
  /* ── Empty + status ──────────────────────────────────── */
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 56px 20px;
    color: var(--text-muted);
    text-align: center;
  }
  .empty-icon {
    width: 52px; height: 52px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, var(--accent) 10%, transparent);
    color: var(--accent);
    margin-bottom: 4px;
    animation: emptyBreathe 3s ease-in-out infinite;
  }
  @keyframes emptyBreathe {
    0%, 100% { transform: scale(1); opacity: 0.9; }
    50%      { transform: scale(1.05); opacity: 1; }
  }
  .empty-title { margin: 0; font-size: 13px; color: var(--text-secondary); font-weight: 500; }
  .empty-hint  { margin: 0; font-size: 11.5px; max-width: 340px; line-height: 1.5; }

  .status {
    padding: 48px 0;
    text-align: center;
    font-size: 12px;
    color: var(--text-muted);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
  }
  .status.error { color: var(--error, #f14c4c); }
  .skeleton {
    display: inline-flex;
    gap: 5px;
  }
  .skeleton span {
    width: 7px; height: 7px;
    border-radius: 50%;
    background: var(--text-muted);
    opacity: 0.35;
    animation: skeletonPulse 1.2s infinite ease-in-out;
  }
  .skeleton span:nth-child(2) { animation-delay: 0.15s; }
  .skeleton span:nth-child(3) { animation-delay: 0.3s; }
  @keyframes skeletonPulse {
    0%, 80%, 100% { opacity: 0.2; transform: scale(0.8); }
    40%           { opacity: 0.95; transform: scale(1); }
  }

  /* ── Graph container ─────────────────────────────────── */
  .graph-wrap {
    background:
      radial-gradient(120% 80% at 50% 0%, color-mix(in srgb, var(--accent) 5%, transparent), transparent 65%),
      var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 14px;
    overflow: hidden;
    position: relative;
  }
  .graph {
    display: block;
    width: 100%;
    height: auto;
    max-height: 560px;
  }

  /* ── Cluster entry + orbit rotation ──────────────────── */
  /*
   * The cluster's outer <g> carries the SVG `transform="translate(..)"`
   * attribute that positions it on the canvas. This inner `.cluster` <g>
   * only uses CSS transforms for the entry animation, so the two don't
   * fight each other (CSS `transform` replaces an SVG attribute wholesale).
   */
  .cluster {
    opacity: 0;
    transform-origin: center;
    transform-box: fill-box;
    transform: scale(0.6);
    transition:
      opacity 420ms cubic-bezier(0.2, 0.8, 0.2, 1) var(--enter-delay, 0ms),
      transform 520ms cubic-bezier(0.2, 0.8, 0.2, 1) var(--enter-delay, 0ms);
  }
  .cluster.entered {
    opacity: 1;
    transform: scale(1);
  }

  .orbit {
    fill: none;
    stroke: color-mix(in srgb, var(--accent) 40%, var(--text-muted));
    stroke-width: 1;
    stroke-dasharray: 3 5;
    opacity: 0.35;
    animation: orbitSpin 40s linear infinite;
    transform-origin: center;
    transform-box: fill-box;
  }
  @keyframes orbitSpin {
    from { transform: rotate(0deg); }
    to   { transform: rotate(-360deg); }
  }

  /* ── Spokes ──────────────────────────────────────────── */
  .spoke {
    stroke-width: 1;
    stroke-dasharray: 1 4;
    opacity: 0.35;
    transition: opacity 180ms ease, stroke-width 180ms ease;
  }
  .spoke.lit {
    opacity: 0.9;
    stroke-width: 1.3;
  }

  /* ── Hub ─────────────────────────────────────────────── */
  .hub { cursor: pointer; outline: none; }
  .hub-ring {
    fill: none;
    stroke: var(--accent);
    stroke-width: 1;
    opacity: 0;
    transition: opacity 220ms ease, stroke-width 220ms ease, r 220ms ease;
  }
  .hub.active .hub-ring,
  .hub:focus-visible .hub-ring {
    opacity: 0.55;
    stroke-width: 1.8;
    animation: hubRingPulse 2.4s ease-in-out infinite;
  }
  @keyframes hubRingPulse {
    0%, 100% { opacity: 0.4; r: 44px; }
    50%      { opacity: 0.75; r: 47px; }
  }
  .hub-bg {
    stroke: color-mix(in srgb, var(--accent) 65%, var(--border));
    stroke-width: 1.5;
    transition:
      stroke-width 180ms ease,
      stroke 180ms ease,
      transform 180ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .hub.active .hub-bg,
  .hub:focus-visible .hub-bg {
    stroke-width: 2.2;
    stroke: var(--accent);
    transform: scale(1.06);
    transform-origin: center;
    transform-box: fill-box;
  }

  .hub-count {
    fill: var(--text-primary);
    font-size: 17px;
    font-weight: 700;
    font-family: var(--font-ui);
    font-variant-numeric: tabular-nums;
    pointer-events: none;
  }
  .hub-title {
    fill: var(--text-secondary);
    font-size: 10px;
    font-weight: 500;
    font-family: var(--font-ui);
    letter-spacing: 0.2px;
    pointer-events: none;
  }

  /* ── Dots ────────────────────────────────────────────── */
  .dot-group { cursor: pointer; outline: none; }
  .dot-glow {
    fill: var(--accent);
    opacity: 0;
    transition: opacity 180ms ease, r 180ms ease;
  }
  .dot-group.active .dot-glow,
  .dot-group:focus-visible .dot-glow {
    opacity: 0.22;
  }
  .dot {
    fill: color-mix(in srgb, var(--text-muted) 90%, var(--bg-surface));
    stroke: var(--bg-secondary);
    stroke-width: 1;
    transition: fill 180ms ease, transform 180ms cubic-bezier(0.2, 0.8, 0.2, 1);
    transform-origin: center;
    transform-box: fill-box;
  }
  .dot-group.active .dot,
  .dot-group:focus-visible .dot {
    fill: var(--accent);
    transform: scale(1.35);
  }

  /* ── Tooltip ─────────────────────────────────────────── */
  .tooltip {
    pointer-events: none;
    animation: tooltipIn 140ms cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  @keyframes tooltipIn {
    from { opacity: 0; transform: translate(0, calc(-1 * var(--dot-r, 7px) - 4px)); }
    to   { opacity: 1; transform: translate(0, calc(-1 * var(--dot-r, 7px) - 10px)); }
  }
  .tooltip-bg {
    fill: var(--bg-secondary);
    stroke: var(--border);
    stroke-width: 1;
    filter: drop-shadow(0 4px 10px rgba(0, 0, 0, 0.25));
  }
  .tooltip-text {
    fill: var(--text-primary);
    font-size: 10.5px;
    font-family: var(--font-ui);
  }

  .more-label {
    fill: var(--text-muted);
    font-size: 10px;
    font-family: var(--font-ui);
    opacity: 0.7;
  }

  /* ── Detail list ─────────────────────────────────────── */
  .details {
    display: flex;
    flex-direction: column;
    gap: 1px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--bg-tertiary);
    overflow: hidden;
    margin-top: 14px;
  }
  .detail-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 11px 14px;
    border-top: 1px solid color-mix(in srgb, var(--border) 35%, transparent);
    transition: background 140ms ease;
  }
  .detail-row:first-child { border-top: none; }
  .detail-row:hover { background: color-mix(in srgb, var(--bg-surface) 60%, transparent); }

  /* Applied briefly when a user clicks the corresponding hub in the graph
     above — pulses an accent glow behind the row so the navigation is
     visible without moving the user's mouse. Scoped :global because the
     class is added imperatively from script, not statically in the template. */
  :global(.detail-row.highlight) {
    animation: rowPulse 1.2s ease-out;
  }
  @keyframes rowPulse {
    0%   { background: color-mix(in srgb, var(--accent) 30%, transparent); box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 55%, transparent); }
    100% { background: transparent; box-shadow: inset 0 0 0 1px transparent; }
  }
  .detail-text { min-width: 0; flex: 1; }
  .detail-title {
    font-size: 12.5px;
    font-weight: 500;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .detail-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 3px;
    font-size: 10.5px;
    color: var(--text-muted);
  }
  .detail-meta :global(svg) { color: var(--text-muted); }
  .dot-sep { opacity: 0.5; }

  .del-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 5px 9px;
    border-radius: 6px;
    background: var(--bg-surface);
    border: 1px solid var(--border);
    color: var(--text-muted);
    font-size: 11px;
    cursor: pointer;
    flex-shrink: 0;
    transition: background 140ms ease, color 140ms ease, border-color 140ms ease, transform 140ms ease;
  }
  .del-btn:hover {
    color: var(--error, #f14c4c);
    border-color: color-mix(in srgb, var(--error, #f14c4c) 45%, var(--border));
    background: color-mix(in srgb, var(--error, #f14c4c) 10%, transparent);
    transform: translateY(-1px);
  }
  .del-btn:active { transform: translateY(0); }

  @media (prefers-reduced-motion: reduce) {
    .orbit { animation: none; }
    .hub.active .hub-ring { animation: none; }
    .empty-icon { animation: none; }
    .cluster { transition-duration: 0ms; }
    .tooltip { animation: none; }
    :global(.detail-row.highlight) { animation: none; background: color-mix(in srgb, var(--accent) 15%, transparent); }
  }
</style>
