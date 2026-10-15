<script lang="ts">
  import type { StateSnapshot, SystemStats } from "../api/state";
  import { formatClock, formatGb, formatPercent, ramPercent } from "../stores/wmState";

  let {
    state,
    stats,
    now,
    onWorkspace,
    onClose,
  }: {
    state: StateSnapshot | null;
    stats: SystemStats | null;
    now: Date;
    onWorkspace: (number: number) => void;
    onClose: () => void;
  } = $props();

  const date = $derived(now.toLocaleDateString("es", { weekday: "long", day: "numeric", month: "long" }));
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onClose()} />

<div class="panel">
  <section class="clock-section">
    <div class="time">{formatClock(now, true)}</div>
    <div class="date">{date}</div>
  </section>

  <section>
    <h3>CPU</h3>
    <div class="meter" role="progressbar" aria-label="CPU" aria-valuenow={stats ? Math.round(stats.cpu_percent) : 0}>
      <div class="fill" style:width={stats ? `${stats.cpu_percent}%` : "0%"}></div>
    </div>
    <div class="value">{stats ? formatPercent(stats.cpu_percent) : "…"}</div>
  </section>

  <section>
    <h3>RAM</h3>
    <div class="meter" role="progressbar" aria-label="RAM" aria-valuenow={stats ? Math.round(ramPercent(stats)) : 0}>
      <div class="fill" style:width={stats ? `${ramPercent(stats)}%` : "0%"}></div>
    </div>
    <div class="value">{stats ? `${formatGb(stats.ram_used)} de ${formatGb(stats.ram_total)}` : "…"}</div>
  </section>

  <section>
    <h3>Workspaces</h3>
    <div class="grid">
      {#each state?.workspaces ?? [] as w (w.number)}
        <button class:visible={w.visible} class:active={w.active} onclick={() => onWorkspace(w.number)}>
          <strong>{w.number}</strong>
          <small>{w.window_count} {w.window_count === 1 ? "ventana" : "ventanas"}</small>
        </button>
      {/each}
    </div>
    {#if !state}<p class="hint">El gestor de ventanas no está activo.</p>{/if}
  </section>
</div>

<style>
  :global(html, body) { margin: 0; height: 100%; background: #14161a; }
  .panel {
    box-sizing: border-box;
    height: 100vh;
    padding: 16px;
    overflow: auto;
    background: #14161a;
    color: #e6e6e6;
    font: 14px/1.3 "Segoe UI", system-ui, sans-serif;
    border: 1px solid #2b2f38;
  }
  section { margin-bottom: 18px; }
  h3 { margin: 0 0 6px; font-size: 12px; text-transform: uppercase; letter-spacing: 0.08em; opacity: 0.6; }
  .time { font-size: 38px; font-weight: 600; font-variant-numeric: tabular-nums; }
  .date { opacity: 0.7; text-transform: capitalize; }
  .meter { height: 8px; border-radius: 4px; background: #2b2f38; overflow: hidden; }
  .fill { height: 100%; background: #3d6df2; transition: width 0.4s; }
  .value { margin-top: 4px; opacity: 0.8; font-variant-numeric: tabular-nums; }
  .grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 6px; }
  .grid button { display: flex; flex-direction: column; align-items: center; gap: 2px; padding: 8px 4px; color: inherit; background: #1d2026; border: 1px solid transparent; border-radius: 8px; cursor: pointer; font: inherit; }
  .grid button.visible { border-color: #3d6df2; }
  .grid button.active { background: #3d6df2; color: #fff; }
  small { opacity: 0.7; }
  .hint { opacity: 0.6; }
</style>
