<script lang="ts">
  import type { StateSnapshot, SystemStats } from "../api/state";
  import type { InstalledModule } from "../api/modules";
  import type { VolumeState } from "../api/system";
  import Icon from "./Icon.svelte";
  import WorkspaceStrip from "./WorkspaceStrip.svelte";
  import Marquee from "./Marquee.svelte";
  import ModuleFrame from "./ModuleFrame.svelte";
  import { formatClock, formatPercent, ramPercent, workspacesFor } from "../stores/wmState";

  let {
    monitorId,
    state,
    stats,
    now,
    volume = null,
    gpu = null,
    temperature = null,
    modules = [],
    onWorkspace,
    onMute = () => {},
  }: {
    monitorId: number;
    state: StateSnapshot | null;
    stats: SystemStats | null;
    now: Date;
    volume?: VolumeState | null;
    gpu?: number | null;
    temperature?: number | null;
    modules?: InstalledModule[];
    onWorkspace: (number: number) => void;
    onMute?: (muted: boolean) => void;
  } = $props();

  const workspaces = $derived(workspacesFor(state, monitorId));
  const barModules = $derived(modules.filter((m) => m.enabled && m.manifest.ui.bar));

  // Datos de la derecha en orden; entre cada par visible va un separador de tres puntos.
  const readouts = $derived([
    ...barModules.map((m) => `mod:${m.manifest.id}`),
    ...(stats ? ["cpu", "ram"] : []),
    ...(gpu !== null ? ["gpu"] : []),
    ...(temperature !== null ? ["temp"] : []),
    ...(volume ? ["vol"] : []),
    "clock",
  ]);
</script>

<div class="bar">
  <div class="zone left">
    <WorkspaceStrip {workspaces} {onWorkspace} />
    {#if state?.mode}<span class="badge">modo {state.mode}</span>{/if}
    {#if state?.paused}<span class="badge warn">en pausa</span>{/if}
  </div>

  <div class="zone center" title={state?.focused?.title ?? ""}>
    {#if state?.focused}
      <div class="title"><Marquee text={state.focused.title || state.focused.exe} /></div>
    {/if}
  </div>

  <div class="zone right">
    {#each readouts as key, i (key)}
      {#if i > 0}<span class="dots" aria-hidden="true"></span>{/if}
      {#if key.startsWith("mod:")}
        {@const m = barModules.find((b) => `mod:${b.manifest.id}` === key)}
        {#if m}<ModuleFrame module={m} height={32} width={m.manifest.ui.bar?.width ?? 120} />{/if}
      {:else if key === "cpu" && stats}
        <span class="stat" title="CPU">CPU {formatPercent(stats.cpu_percent)}</span>
      {:else if key === "ram" && stats}
        <span class="stat" title="RAM">RAM {formatPercent(ramPercent(stats))}</span>
      {:else if key === "gpu" && gpu !== null}
        <span class="stat" title="Tarjeta gráfica">GPU {formatPercent(gpu)}</span>
      {:else if key === "temp" && temperature !== null}
        <span class="stat" title="Temperatura en Mérida">Mérida {Math.round(temperature)} °C</span>
      {:else if key === "vol" && volume}
        <button class="stat vol" aria-label={volume.muted ? "Activar sonido" : "Silenciar"} onclick={() => onMute(!volume.muted)}>
          <Icon name={volume.muted ? "volume-off" : "volume"} size={14} /> {Math.round(volume.percent)}%
        </button>
      {:else if key === "clock"}
        <span class="clock">{formatClock(now)}</span>
      {/if}
    {/each}
  </div>
</div>

<style>
  :global(html, body) { margin: 0; height: 100%; overflow: hidden; background: transparent; }
  .bar {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: center;
    height: 100vh;
    padding: 0 10px;
    box-sizing: border-box;
    background-color: rgba(10, 10, 11, 0.58);
    border-bottom: 1px solid rgba(255, 255, 255, 0.12);
    color: #f1f1f1;
    font: 12px/1 "Cascadia Mono", Consolas, ui-monospace, monospace;
    letter-spacing: 0.04em;
    user-select: none;
  }
  .zone { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .left { justify-content: flex-start; position: relative; }
  .center { justify-content: center; overflow: hidden; opacity: 0.85; }
  .title { width: clamp(120px, 30vw, 560px); min-width: 0; }
  .right { justify-content: flex-end; gap: 10px; }
  button { font: inherit; color: inherit; background: transparent; border: 0; cursor: pointer; border-radius: 6px; }
  /* Tres puntos apilados, estilo Nothing, entre dato y dato. */
  .dots {
    flex: none; width: 4px; height: 15px; opacity: 0.35;
    background: radial-gradient(circle, currentColor 1px, transparent 1.4px) 50% 0 / 4px 5px repeat-y;
  }
  .badge { padding: 2px 8px; border-radius: 999px; background: rgba(255, 255, 255, 0.12); font-size: 11px; }
  .badge.warn { background: #f2f2f2; color: #0a0a0b; }
  .stat { opacity: 0.8; font-variant-numeric: tabular-nums; }
  .vol { padding: 2px 4px; }
  .clock { font-weight: 600; font-variant-numeric: tabular-nums; }
  .vol:hover { background: rgba(255, 255, 255, 0.14); }
</style>
