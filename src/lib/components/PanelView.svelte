<script lang="ts">
  import "../styles/panel.css";
  import type { StateSnapshot } from "../api/state";
  import type {
    ActionOutcome,
    MediaCommandName,
    MediaInfo,
    PowerActionName,
    RadioKindName,
    RadioState,
    SystemOverview,
    VolumeState,
    WallpaperList,
  } from "../api/system";
  import { formatClock } from "../stores/wmState";
  import type { InstalledModule } from "../api/modules";
  import ModuleSections from "./ModuleSections.svelte";
  import ConnectivitySection from "./sections/ConnectivitySection.svelte";
  import MediaSection from "./sections/MediaSection.svelte";
  import PowerSection from "./sections/PowerSection.svelte";
  import StatsSection from "./sections/StatsSection.svelte";
  import WallpaperSection from "./sections/WallpaperSection.svelte";

  let {
    state,
    now,
    overview,
    media,
    volume,
    radios,
    wallpapers,
    allowChanges,
    modules = [],
    onWorkspace,
    onClose,
    onMediaCommand,
    onVolume,
    onMute,
    onRadio,
    onPower,
    onWallpaper,
  }: {
    state: StateSnapshot | null;
    now: Date;
    overview: SystemOverview | null;
    media: MediaInfo | null;
    volume: VolumeState | null;
    radios: RadioState[];
    wallpapers: WallpaperList | null;
    allowChanges: boolean;
    modules?: InstalledModule[];
    onWorkspace: (number: number) => void;
    onClose: () => void;
    onMediaCommand: (command: MediaCommandName) => void;
    onVolume: (percent: number) => void;
    onMute: (muted: boolean) => void;
    onRadio: (kind: RadioKindName, on: boolean) => Promise<ActionOutcome>;
    onPower: (action: PowerActionName) => Promise<ActionOutcome>;
    onWallpaper: (name: string) => Promise<ActionOutcome>;
  } = $props();

  const date = $derived(now.toLocaleDateString("es", { weekday: "long", day: "numeric", month: "long" }));
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onClose()} />

<div class="panel">
  <section class="section clock-section">
    <div class="time">{formatClock(now, true)}</div>
    <div class="date">{date}</div>
  </section>

  <MediaSection {media} {volume} onCommand={onMediaCommand} {onVolume} {onMute} />

  <StatsSection {overview} />

  <section class="section">
    <h3>Workspaces</h3>
    <div class="grid">
      {#each state?.workspaces ?? [] as w (w.number)}
        <button class="ws" class:visible={w.visible} class:active={w.active} onclick={() => onWorkspace(w.number)}>
          <strong>{w.number}</strong>
          <small>{w.window_count} {w.window_count === 1 ? "ventana" : "ventanas"}</small>
        </button>
      {/each}
    </div>
    {#if !state}<p class="hint">El gestor de ventanas no está activo.</p>{/if}
  </section>

  <ConnectivitySection {radios} {allowChanges} onSet={onRadio} />
  <WallpaperSection list={wallpapers} onSet={onWallpaper} />
  <PowerSection {allowChanges} onRun={onPower} />
  <ModuleSections {modules} />
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
  .time { font-size: 38px; font-weight: 600; font-variant-numeric: tabular-nums; }
  .date { opacity: 0.7; }
  .date::first-letter { text-transform: uppercase; }
  .grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 6px; }
  .ws { display: flex; flex-direction: column; align-items: center; gap: 2px; padding: 8px 4px; color: inherit; background: #1d2026; border: 1px solid transparent; border-radius: 8px; cursor: pointer; font: inherit; }
  .ws.visible { border-color: #3d6df2; }
  .ws.active { background: #3d6df2; color: #fff; }
  small { opacity: 0.7; }
</style>
