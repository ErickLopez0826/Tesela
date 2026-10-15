<script lang="ts">
  import type { MediaCommandName, MediaInfo, VolumeState } from "../../api/system";

  let {
    media,
    volume,
    onCommand,
    onVolume,
    onMute,
  }: {
    media: MediaInfo | null;
    volume: VolumeState | null;
    onCommand: (command: MediaCommandName) => void;
    onVolume: (percent: number) => void;
    onMute: (muted: boolean) => void;
  } = $props();
</script>

<section class="section">
  <h3>Medios</h3>
  {#if media}
    <div class="now">
      <strong>{media.title}</strong>
      <div class="hint">{media.artist}{media.app ? ` · ${media.app}` : ""}</div>
    </div>
  {:else}
    <div class="hint">No suena nada.</div>
  {/if}
  <div class="buttons controls">
    <button class="btn" aria-label="Anterior" onclick={() => onCommand("previous")} disabled={!media}>⏮</button>
    <button class="btn primary" aria-label={media?.playing ? "Pausar" : "Reproducir"} onclick={() => onCommand("play_pause")} disabled={!media}>
      {media?.playing ? "⏸" : "▶"}
    </button>
    <button class="btn" aria-label="Siguiente" onclick={() => onCommand("next")} disabled={!media}>⏭</button>
  </div>

  {#if volume}
    <div class="row volume">
      <button class="btn" aria-label={volume.muted ? "Activar sonido" : "Silenciar"} onclick={() => onMute(!volume.muted)}>
        {volume.muted ? "🔇" : "🔊"}
      </button>
      <input
        type="range"
        min="0"
        max="100"
        value={Math.round(volume.percent)}
        aria-label="Volumen"
        onchange={(e) => onVolume(Number((e.currentTarget as HTMLInputElement).value))}
      />
      <span class="value">{Math.round(volume.percent)}%</span>
    </div>
  {/if}
</section>

<style>
  .controls { margin-top: 8px; }
  .volume { margin-top: 8px; }
  .volume input { flex: 1; }
  .now strong { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
