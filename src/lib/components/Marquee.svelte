<script lang="ts">
  // Texto en una sola línea: si cabe se queda quieto y centrado; si no, va y viene por todo el espacio disponible.
  let { text }: { text: string } = $props();

  const SPEED_PX_PER_S = 35;
  const MIN_SECONDS = 4;

  let box: HTMLElement | undefined = $state();
  let inner: HTMLElement | undefined = $state();
  let shift = $state(0);

  function measure() {
    if (!box || !inner) return;
    shift = Math.max(0, Math.ceil(inner.scrollWidth - box.clientWidth));
  }

  $effect(() => {
    void text;
    measure();
    if (!box || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(box);
    return () => observer.disconnect();
  });

  const seconds = $derived(Math.max(MIN_SECONDS, shift / SPEED_PX_PER_S));
  // Fotogramas del recorrido: unos 25 por segundo. Cada fotograma que se produce tiene un coste fijo, y a esa
  // velocidad un texto que avanza a 35 px/s se ve igual de fluido.
  const steps = $derived(Math.max(8, Math.round(seconds * 0.76 * 25)));
</script>

<div class="marquee" class:scrolls={shift > 0} bind:this={box}>
  {#key text}
    <span class="text" bind:this={inner} style:--shift={`${shift}px`} style:--dur={`${seconds}s`} style:--steps={steps}>{text}</span>
  {/key}
</div>

<style>
  .marquee { overflow: hidden; white-space: nowrap; text-align: center; min-width: 0; width: 100%; }
  .text { display: inline-block; }
  .scrolls { text-align: left; -webkit-mask-image: linear-gradient(90deg, transparent, #000 8%, #000 92%, transparent); mask-image: linear-gradient(90deg, transparent, #000 8%, #000 92%, transparent); }
  .scrolls .text { animation: slide var(--dur) steps(var(--steps)) infinite alternate; will-change: transform; }
  @keyframes slide {
    0%, 12% { transform: translateX(0); }
    88%, 100% { transform: translateX(calc(var(--shift) * -1)); }
  }
  @media (prefers-reduced-motion: reduce) {
    .scrolls { -webkit-mask-image: none; mask-image: none; text-overflow: ellipsis; }
    .scrolls .text { animation: none; }
  }
</style>
