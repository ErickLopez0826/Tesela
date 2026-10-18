<script lang="ts">
  import Icon from "../Icon.svelte";
  import Slider from "../Slider.svelte";
  import type { MediaCommandName, MediaInfo, VolumeState } from "../../api/system";

  let {
    media,
    volume,
    art = null,
    onCommand,
    onVolume,
    onMute,
  }: {
    media: MediaInfo | null;
    volume: VolumeState | null;
    /// Portada o miniatura de lo que suena (data URL), si Windows la ofrece.
    art?: string | null;
    onCommand: (command: MediaCommandName) => void;
    onVolume: (percent: number) => void;
    onMute: (muted: boolean) => void;
  } = $props();

  // Al cambiar de canción la portada nueva aparece suavemente mientras la anterior se desvanece.
  const ART_MS = 420;
  let leavingArt: string | null = $state(null);
  let shownArt: string | null = null;
  let artTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const next = art;
    if (next === shownArt) return;
    leavingArt = shownArt;
    shownArt = next;
    clearTimeout(artTimer);
    artTimer = setTimeout(() => (leavingArt = null), ART_MS);
  });
  $effect(() => () => clearTimeout(artTimer));

  // Mientras se arrastra, el volumen sigue al deslizador (como mucho cada 120 ms); al soltar se aplica el valor final.
  let lastSent = 0;
  let live: number | null = $state(null); // valor mostrado mientras se arrastra

  function slide(value: number) {
    live = value;
    const now = Date.now();
    if (now - lastSent < 120) return;
    lastSent = now;
    onVolume(value);
  }
</script>

<section class="section">
  <h3>Medios</h3>
  <div class="card">
    <div class="art" class:has-art={!!art || !!leavingArt} aria-hidden="true">
      {#if leavingArt}<img class="out" src={leavingArt} alt="" />{/if}
      {#if art}{#key art}<img class="in" src={art} alt="" />{/key}{:else if !leavingArt}<Icon name="music" size={22} />{/if}
    </div>
    <div class="now">
      {#if media}
        <strong title={media.title}>{media.title}</strong>
        <div class="hint">{media.artist}{media.app ? ` · ${media.app}` : ""}</div>
      {:else}
        <div class="hint">No suena nada.</div>
      {/if}
    </div>
    <div class="controls">
      <button class="round" aria-label="Anterior" onclick={() => onCommand("previous")} disabled={!media}><Icon name="previous" size={14} /></button>
      <button class="round main" aria-label={media?.playing ? "Pausar" : "Reproducir"} onclick={() => onCommand("play_pause")} disabled={!media}>
        <Icon name={media?.playing ? "pause" : "play"} size={16} />
      </button>
      <button class="round" aria-label="Siguiente" onclick={() => onCommand("next")} disabled={!media}><Icon name="next" size={14} /></button>
    </div>
  </div>

  {#if volume}
    <div class="volume">
      <button class="round" aria-label={volume.muted ? "Activar sonido" : "Silenciar"} onclick={() => onMute(!volume.muted)}>
        <Icon name={volume.muted ? "volume-off" : "volume"} size={14} />
      </button>
      <Slider
        value={live ?? Math.round(volume.percent)}
        label="Volumen"
        oninput={slide}
        onchange={(v) => {
          onVolume(v);
          live = null;
        }}
      />
      <span class="value">{live ?? Math.round(volume.percent)}%</span>
    </div>
  {/if}
</section>

<style>
  .card { display: flex; align-items: center; gap: 12px; padding: 10px; border-radius: 18px; background: rgba(255, 255, 255, 0.05); border: 1px solid var(--line); }
  .art { position: relative; display: grid; place-items: center; flex: none; width: 56px; height: 56px; overflow: hidden; border-radius: 14px; background: rgba(255, 255, 255, 0.08); color: var(--muted); }
  .art.has-art { background: #000; box-shadow: 0 6px 18px -8px rgba(255, 255, 255, 0.35); }
  .art img { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; display: block; }
  /* Cambio de canción: la portada nueva entra con un ligero zoom mientras la anterior se desvanece. */
  .art img.in { animation: art-in 0.42s cubic-bezier(.22, 1, .36, 1) both; }
  .art img.out { animation: art-out 0.42s ease both; }
  @keyframes art-in { from { opacity: 0; transform: scale(1.12); } to { opacity: 1; transform: none; } }
  @keyframes art-out { to { opacity: 0; transform: scale(0.94); } }
  .now { flex: 1; min-width: 0; }
  .now strong { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font: 600 14px "Segoe UI", system-ui, sans-serif; }
  .controls { display: flex; gap: 6px; align-items: center; }
  .round { display: grid; place-items: center; width: 30px; height: 30px; padding: 0; border-radius: 50%; color: var(--fg); background: rgba(255, 255, 255, 0.08); border: 1px solid var(--line); cursor: pointer; transition: background 0.3s, transform 0.4s cubic-bezier(.3, 1.8, .5, 1); }
  .round:hover:not(:disabled) { background: rgba(255, 255, 255, 0.18); transform: scale(1.12); }
  .round:active:not(:disabled) { transform: scale(0.88); }
  .round:disabled { opacity: 0.35; cursor: default; }
  .round.main { width: 38px; height: 38px; background: var(--accent); border-color: var(--accent); color: var(--on-accent); box-shadow: 0 0 14px -4px var(--accent); }
  .round.main:hover:not(:disabled) { background: var(--accent); filter: brightness(1.1); }

  .volume { display: flex; align-items: center; gap: 10px; margin-top: 10px; }
  .value { font: 12px var(--mono); min-width: 36px; text-align: right; }
  @media (prefers-reduced-motion: reduce) { .art img { animation: none; } }
</style>
