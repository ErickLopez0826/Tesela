<!-- Pila con su carga dibujada por dentro; con el cargador puesto lleva un rayo encima. Hereda el color del texto. -->
<script lang="ts">
  let { percent, charging = false, size = 18 }: { percent: number; charging?: boolean; size?: number } = $props();

  // El relleno ocupa el hueco interior (14 de ancho): con poca carga sigue viéndose una franja mínima.
  const fill = $derived(percent <= 0 ? 0 : Math.max(1.5, (Math.min(100, percent) / 100) * 14));
</script>

<svg class="battery" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  <rect x="2" y="7" width="17" height="10" rx="2.5" />
  <path d="M22 11v2" />
  <rect x="4" y="9" width={fill} height="6" rx="1" fill="currentColor" stroke="none" data-fill />
  {#if charging}
    <path class="bolt" d="M12.5 5.5 8 12.5h4l-.5 6 4.5-7h-4z" fill="currentColor" stroke="var(--bolt-edge, #0a0a0b)" stroke-width="1.2" />
  {/if}
</svg>
