<!-- Anillo de puntos con la marca de Claude en el centro: los puntos encendidos son lo gastado (como los del resto de Tesela, no un trazo
     continuo). El color sigue lo cerca que estás del límite. -->
<script lang="ts">
  import type { UsageLevel } from "../api/claude";
  import ClaudeMark from "./ClaudeMark.svelte";

  let {
    percent,
    level = "ok",
    size = 26,
    dim = false,
    working = false,
  }: {
    /** 0 a 100, o `null` si no hay dato vigente: el anillo queda apagado. */
    percent: number | null;
    level?: UsageLevel;
    size?: number;
    /** Lectura vieja: todo atenuado. */
    dim?: boolean;
    /** Claude está trabajando: los puntos se encienden en cadena alrededor del anillo y la marca respira. */
    working?: boolean;
  } = $props();

  const DOTS = 16;
  /** Puntos encendidos: al menos uno si se ha gastado algo, y todos solo al 100 %. */
  const lit = $derived(percent === null || percent <= 0 ? 0 : Math.min(DOTS, Math.max(1, Math.round((percent / 100) * DOTS))));
  const dots = Array.from({ length: DOTS }, (_, i) => {
    const angle = (i / DOTS) * 2 * Math.PI;
    return { cx: 16 + Math.sin(angle) * 13, cy: 16 - Math.cos(angle) * 13 };
  });
</script>

<span class="usage-ring {level}" class:dim class:working style:width={`${size}px`} style:height={`${size}px`} aria-hidden="true">
  <svg viewBox="0 0 32 32" width={size} height={size}>
    {#each dots as d, i (i)}
      <circle class="dot" class:on={i < lit} style:--i={i} cx={d.cx} cy={d.cy} r="1.7" />
    {/each}
  </svg>
  <span class="mark"><ClaudeMark size={Math.round(size * 0.46)} /></span>
</span>

<style>
  .usage-ring { position: relative; display: inline-grid; place-items: center; flex: none; }
  svg { position: absolute; inset: 0; }
  .dot { fill: currentColor; opacity: 0.22; transition: opacity 0.3s ease, fill 0.3s ease; }
  .dot.on { opacity: 1; }
  .warn .dot.on { fill: #ffb347; }
  .danger .dot.on { fill: #ff6b5e; }
  .mark { position: relative; display: grid; place-items: center; color: #d97757; }
  .dim { opacity: 0.5; }
  /* Trabajando: una ola de puntos que da la vuelta al anillo (16 puntos × 80 ms = una vuelta cada 1,28 s). */
  .working .dot { transform-box: fill-box; transform-origin: center; animation: ring-chase 1.28s ease-in-out infinite; animation-delay: calc(var(--i) * 80ms); }
  .working .mark { animation: mark-breathe 1.6s ease-in-out infinite; }
  @keyframes ring-chase { 0%, 55%, 100% { transform: scale(1); } 20% { transform: scale(1.75); opacity: 1; } }
  @keyframes mark-breathe { 50% { transform: scale(0.82); opacity: 0.75; } }
  @media (prefers-reduced-motion: reduce) { .working .dot, .working .mark { animation: none; } }
</style>
