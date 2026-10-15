<script lang="ts">
  import type { StateSnapshot, SystemStats } from "../api/state";
  import { formatClock, formatPercent, ramPercent, workspacesFor } from "../stores/wmState";

  let {
    monitorId,
    state,
    stats,
    now,
    onWorkspace,
    onPanel,
  }: {
    monitorId: number;
    state: StateSnapshot | null;
    stats: SystemStats | null;
    now: Date;
    onWorkspace: (number: number) => void;
    onPanel: () => void;
  } = $props();

  const workspaces = $derived(workspacesFor(state, monitorId));
</script>

<div class="bar">
  <div class="zone left">
    {#each workspaces as w (w.number)}
      <button class="ws" class:visible={w.visible} class:active={w.active} class:busy={w.window_count > 0} onclick={() => onWorkspace(w.number)}>
        {w.number}
      </button>
    {/each}
    {#if state?.mode}<span class="badge">modo {state.mode}</span>{/if}
    {#if state?.paused}<span class="badge warn">en pausa</span>{/if}
  </div>

  <div class="zone center" title={state?.focused?.title ?? ""}>
    {#if state?.focused}{state.focused.title || state.focused.exe}{/if}
  </div>

  <div class="zone right">
    {#if stats}
      <span class="stat" title="CPU">CPU {formatPercent(stats.cpu_percent)}</span>
      <span class="stat" title="RAM">RAM {formatPercent(ramPercent(stats))}</span>
    {/if}
    <span class="clock">{formatClock(now)}</span>
    <button class="panel-btn" aria-label="Abrir panel" onclick={onPanel}>☰</button>
  </div>
</div>

<style>
  :global(html, body) { margin: 0; height: 100%; overflow: hidden; background: #111; }
  .bar {
    display: grid;
    grid-template-columns: 1fr minmax(0, 2fr) 1fr;
    align-items: center;
    height: 100vh;
    padding: 0 10px;
    box-sizing: border-box;
    background: #14161a;
    color: #e6e6e6;
    font: 13px/1 "Segoe UI", system-ui, sans-serif;
    user-select: none;
  }
  .zone { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .left { justify-content: flex-start; }
  .center { justify-content: center; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; opacity: 0.85; }
  .right { justify-content: flex-end; }
  button { font: inherit; color: inherit; background: transparent; border: 0; cursor: pointer; border-radius: 6px; }
  .ws { min-width: 24px; height: 24px; opacity: 0.45; }
  .ws.busy { opacity: 0.8; }
  .ws.visible { opacity: 1; background: #2b2f38; }
  .ws.active { background: #3d6df2; color: #fff; }
  .badge { padding: 2px 6px; border-radius: 6px; background: #2b2f38; font-size: 11px; }
  .badge.warn { background: #7a4b00; }
  .stat { opacity: 0.8; font-variant-numeric: tabular-nums; }
  .clock { font-weight: 600; font-variant-numeric: tabular-nums; }
  .panel-btn { padding: 2px 8px; font-size: 15px; }
  .panel-btn:hover, .ws:hover { background: #2b2f38; }
</style>
