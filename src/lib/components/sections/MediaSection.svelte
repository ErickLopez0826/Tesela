<script lang="ts">
  import Icon from "../Icon.svelte";
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

  // Mientras se arrastra, el volumen sigue al deslizador (como mucho cada 120 ms); al soltar se aplica el valor final.
  let lastSent = 0;
  let live: number | null = $state(null); // valor mostrado mientras se arrastra
  let slider: HTMLInputElement | undefined = $state();
  let lastValue: number | null = null;
  let relax: ReturnType<typeof setTimeout> | undefined;

  // La bolita se estira en el sentido del arrastre según la velocidad y se relaja al parar, como una gota.
  function stretch(value: number) {
    if (!slider) return;
    const speed = lastValue === null ? 0 : Math.abs(value - lastValue);
    lastValue = value;
    slider.parentElement?.style.setProperty("--stretch", String(1 + Math.min(1.1, speed / 5)));
    clearTimeout(relax);
    relax = setTimeout(() => slider?.parentElement?.style.setProperty("--stretch", "1"), 90);
  }

  function slide(value: number) {
    live = value;
    stretch(value);
    const now = Date.now();
    if (now - lastSent < 120) return;
    lastSent = now;
    onVolume(value);
  }
</script>

<section class="section">
  <h3>Medios</h3>
  <div class="card">
    <div class="art" class:has-art={!!art} aria-hidden="true">
      {#if art}<img src={art} alt="" />{:else}<Icon name="music" size={22} />{/if}
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
      <div class="track" class:dragging={live !== null} style:--p={`${live ?? Math.round(volume.percent)}%`} style:--pn={live ?? Math.round(volume.percent)}>
        <div class="fill"></div>
        <i class="knob"></i>
        <input
          bind:this={slider}
          type="range"
          min="0"
          max="100"
          value={live ?? Math.round(volume.percent)}
          aria-label="Volumen"
          oninput={(e) => slide(Number((e.currentTarget as HTMLInputElement).value))}
          onchange={(e) => {
            onVolume(Number((e.currentTarget as HTMLInputElement).value));
            live = null;
            lastValue = null;
          }}
        />
      </div>
      <span class="value">{live ?? Math.round(volume.percent)}%</span>
    </div>
  {/if}
</section>

<style>
  .card { display: flex; align-items: center; gap: 12px; padding: 10px; border-radius: 18px; background: rgba(255, 255, 255, 0.05); border: 1px solid var(--line); }
  .art { display: grid; place-items: center; flex: none; width: 56px; height: 56px; overflow: hidden; border-radius: 14px; background: rgba(255, 255, 255, 0.08); color: var(--muted); }
  .art.has-art { background: #000; box-shadow: 0 6px 18px -8px rgba(255, 255, 255, 0.35); animation: art-in 0.5s cubic-bezier(.2, .9, .2, 1) both; }
  .art img { width: 100%; height: 100%; object-fit: cover; display: block; }
  @keyframes art-in { from { opacity: 0; transform: scale(0.85) rotate(-4deg); } to { opacity: 1; transform: none; } }
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
  /* Volumen «líquido»: la pista y el relleno se dibujan aparte del control nativo (que queda encima, invisible, para
     arrastrar y para el teclado). El relleno mide exactamente el porcentaje: al 100 % llena la pista entera. */
  .track {
    --stretch: 1;
    --thumb: 24px;
    /* Posición del centro de la bolita: recorre la pista de borde a borde sin salirse (en 0 % y en 100 % queda dentro). */
    --x: calc(var(--p) - var(--thumb) * var(--pn) / 100 + var(--thumb) / 2);
    position: relative; flex: 1; height: 16px; border-radius: 999px;
    background: rgba(255, 255, 255, 0.16); box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.22);
  }
  .fill {
    position: absolute; inset: 0 auto 0 0; width: var(--x); border-radius: 999px;
    background: var(--accent);
    box-shadow: 0 0 14px -2px var(--accent);
    transition: width 0.5s cubic-bezier(.2, 1.45, .35, 1);
  }
  /* Destello que recorre el relleno solo mientras se toca (así no hay animación continua en reposo). */
  .fill::after {
    content: ""; position: absolute; inset: 0; border-radius: inherit; opacity: 0; transition: opacity 0.3s;
    background: linear-gradient(105deg, transparent 35%, rgba(255, 255, 255, 0.5) 50%, transparent 65%) 0 0 / 220% 100% no-repeat;
  }
  .track:hover .fill::after, .track.dragging .fill::after { opacity: 1; animation: shimmer 2.4s ease-in-out infinite; }
  .track.dragging .fill { transition: none; }
  @keyframes shimmer { from { background-position: 130% 0; } to { background-position: -130% 0; } }
  /* La bolita: una perla blanca con un punto del color del tema; crece al pasar el cursor y se aprieta al arrastrar.
     Se estira en el sentido del arrastre según la velocidad y se relaja al parar, como una gota. */
  .knob {
    position: absolute; top: 50%; left: calc(var(--x) - var(--thumb) / 2); width: var(--thumb); height: var(--thumb); margin-top: calc(var(--thumb) / -2);
    border-radius: 50%; box-sizing: border-box; pointer-events: none;
    background: radial-gradient(circle at 35% 30%, #ffffff 0%, #ececf0 70%, #d4d4da 100%);
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.55), 0 0 0 1px rgba(0, 0, 0, 0.25), 0 0 14px -2px var(--accent);
    transform: scaleX(var(--stretch)) scaleY(calc(2 - var(--stretch) * 0.6));
    transition: left 0.5s cubic-bezier(.2, 1.45, .35, 1), transform 0.35s cubic-bezier(.3, 2, .5, 1), box-shadow 0.3s;
  }
  .knob::after { content: ""; position: absolute; inset: 7px; border-radius: 50%; background: var(--accent); box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.25); }
  .track:hover .knob { box-shadow: 0 3px 8px rgba(0, 0, 0, 0.6), 0 0 0 1px rgba(0, 0, 0, 0.25), 0 0 18px 0 var(--accent); }
  .track.dragging .knob { transition: left 0s, transform 0.35s cubic-bezier(.3, 2, .5, 1); box-shadow: 0 1px 4px rgba(0, 0, 0, 0.6), 0 0 0 4px color-mix(in srgb, var(--accent) 35%, transparent), 0 0 20px 0 var(--accent); }
  .track input { position: absolute; inset: 0; width: 100%; height: 100%; margin: 0; opacity: 0; cursor: pointer; }
  .value { font: 12px var(--mono); min-width: 36px; text-align: right; }
  @media (prefers-reduced-motion: reduce) { .fill, .knob { transition: none; } .fill::after { animation: none !important; } .art.has-art { animation: none; } }
</style>
