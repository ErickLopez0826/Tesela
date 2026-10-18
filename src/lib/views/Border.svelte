<!-- Borde de la ventana en uso: anillo que palpita y un destello que recorre el contorno.
     Las dos animaciones solo tocan `opacity` y `transform` (las resuelve la tarjeta gráfica sin repintar nada): antes
     animaban una sombra y un degradado cónico, que obligaban a repintar el borde entero (casi una pantalla) en cada
     fotograma y mantenían ocupado al proceso gráfico de WebView2. Además van «a saltos» de 40 ms (25 fotogramas por
     segundo): a esa velocidad un resplandor lento se ve igual de suave y se produce menos de la mitad de fotogramas
     (cada uno cuesta lo mismo aunque cambie poco). Las dos duran lo mismo para que sus saltos coincidan. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { theme } from "../stores/theme";

  // El color («r, g, b») llega como una cadena, que CSS no puede animar: se reparte en tres números registrados
  // (`--ring-r`, `--ring-g`, `--ring-b`) y el anillo los transiciona, así el color cambia de forma suave.
  let ring: HTMLElement | undefined = $state();
  let channels = $state([255, 255, 255]);
  let ready = $state(false);

  function read() {
    const raw = getComputedStyle(document.documentElement).getPropertyValue("--frame");
    const parts = raw.split(",").map((p) => Number.parseInt(p.trim(), 10));
    if (parts.length === 3 && parts.every((n) => Number.isFinite(n))) channels = parts;
  }

  onMount(() => {
    read();
    // La primera lectura se pinta sin transición; después, el color viaja suave.
    requestAnimationFrame(() => (ready = true));
    window.addEventListener("wm-theme-applied", read);
    return () => window.removeEventListener("wm-theme-applied", read);
  });
</script>

<!-- «Sin marco» no dibuja nada; «fijo» deja el anillo quieto; «animado» añade el pulso y el destello que recorre el contorno. -->
{#if $theme.borderStyle !== "off"}
  <div class="ring" class:steady={$theme.borderStyle === "steady"} class:ready bind:this={ring} style:--ring-r={channels[0]} style:--ring-g={channels[1]} style:--ring-b={channels[2]}>
    <div class="glow"></div>
    {#if $theme.borderStyle === "animated"}<div class="trail"><div class="sweep"></div></div>{/if}
  </div>
{/if}

<style>
  :global(html, body) { margin: 0; height: 100%; background: transparent; overflow: hidden; }
  @property --ring-r { syntax: "<number>"; inherits: true; initial-value: 255; }
  @property --ring-g { syntax: "<number>"; inherits: true; initial-value: 255; }
  @property --ring-b { syntax: "<number>"; inherits: true; initial-value: 255; }
  .ring { position: fixed; inset: 1px; border-radius: 12px; box-sizing: border-box; --frame-rgb: var(--ring-r), var(--ring-g), var(--ring-b); }
  .ring.ready { transition: --ring-r 0.9s ease, --ring-g 0.9s ease, --ring-b 0.9s ease; }
  /* Anillo fijo con su resplandor; solo palpita su opacidad. */
  .glow {
    position: absolute; inset: 0; border-radius: 12px; box-sizing: border-box;
    border: 2px solid rgba(var(--frame-rgb), 0.6);
    box-shadow: 0 0 12px rgba(var(--frame-rgb), 0.4), inset 0 0 9px rgba(var(--frame-rgb), 0.18);
    animation: pulse 3.2s steps(80) infinite;
    will-change: opacity;
  }
  /* El destello: un degradado cónico dentro de un recorte con forma de anillo. El recorte no cambia; lo que gira es el
     degradado (transform), y como el recorte es del tamaño del anillo, solo se ve la franja del contorno. */
  .trail {
    position: absolute; inset: -2px; border-radius: 12px; padding: 2px; overflow: hidden;
    -webkit-mask: linear-gradient(#000 0 0) content-box, linear-gradient(#000 0 0);
    -webkit-mask-composite: xor; mask-composite: exclude;
  }
  .sweep {
    --d: calc(max(100vw, 100vh) * 1.5);
    position: absolute; left: 50%; top: 50%; width: var(--d); height: var(--d); margin: calc(var(--d) / -2) 0 0 calc(var(--d) / -2);
    background: conic-gradient(transparent 0 62%, rgba(var(--frame-rgb), 0.2) 78%, rgb(var(--frame-rgb)) 96%, transparent 100%);
    animation: travel 3.2s steps(80) infinite;
    will-change: transform;
  }
  .steady .glow { animation: none; }
  @keyframes pulse { 50% { opacity: 0.4; } }
  @keyframes travel { to { transform: rotate(360deg); } }
  @media (prefers-reduced-motion: reduce) { .glow, .sweep { animation: none; } .ring.ready { transition: none; } }
</style>
