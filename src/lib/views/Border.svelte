<!-- Borde de la ventana en uso: anillo que palpita y un destello que recorre el contorno.
     Las dos animaciones solo tocan `opacity` y `transform` (las resuelve la tarjeta gráfica sin repintar nada): antes
     animaban una sombra y un degradado cónico, que obligaban a repintar el borde entero (casi una pantalla) en cada
     fotograma y mantenían ocupado al proceso gráfico de WebView2. Además van «a saltos» de 40 ms (25 fotogramas por
     segundo): a esa velocidad un resplandor lento se ve igual de suave y se produce menos de la mitad de fotogramas
     (cada uno cuesta lo mismo aunque cambie poco). Las dos duran lo mismo para que sus saltos coincidan. -->
<script lang="ts">
  import { theme } from "../stores/theme";
</script>

<!-- «Sin marco» no dibuja nada; «fijo» deja el anillo quieto; «animado» añade el pulso y el destello que recorre el contorno. -->
{#if $theme.borderStyle !== "off"}
  <div class="ring" class:steady={$theme.borderStyle === "steady"}>
    <div class="glow"></div>
    {#if $theme.borderStyle === "animated"}<div class="trail"><div class="sweep"></div></div>{/if}
  </div>
{/if}

<style>
  :global(html, body) { margin: 0; height: 100%; background: transparent; overflow: hidden; }
  .ring { position: fixed; inset: 1px; border-radius: 12px; box-sizing: border-box; }
  /* Anillo fijo con su resplandor; solo palpita su opacidad. */
  .glow {
    position: absolute; inset: 0; border-radius: 12px; box-sizing: border-box;
    border: 2px solid rgba(var(--frame, 255, 255, 255), 0.6);
    box-shadow: 0 0 12px rgba(var(--frame, 255, 255, 255), 0.4), inset 0 0 9px rgba(var(--frame, 255, 255, 255), 0.18);
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
    background: conic-gradient(transparent 0 62%, rgba(var(--frame, 255, 255, 255), 0.2) 78%, rgb(var(--frame, 255, 255, 255)) 96%, transparent 100%);
    animation: travel 3.2s steps(80) infinite;
    will-change: transform;
  }
  .steady .glow { animation: none; }
  @keyframes pulse { 50% { opacity: 0.4; } }
  @keyframes travel { to { transform: rotate(360deg); } }
  @media (prefers-reduced-motion: reduce) { .glow, .sweep { animation: none; } }
</style>
