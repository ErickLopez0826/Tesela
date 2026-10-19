<!-- Logo de una página web: su favicon y, si no se consigue, un icono genérico. -->
<script lang="ts">
  import Icon, { type IconName } from "./Icon.svelte";
  import { failedCount, faviconSources, rememberFailed } from "../stores/favicon";

  let { site, fallback = "globe", size = 28 }: { site: string; fallback?: IconName; size?: number } = $props();

  const sources = $derived(faviconSources(site));
  // Cuántas fuentes han fallado para esta página (la memoria global evita volver a probar las que no sirven).
  let local = $state({ site: "", count: 0 });
  const failed = $derived(local.site === site ? local.count : failedCount(site));
  const src = $derived(sources[failed]);

  function onError() {
    const count = failed + 1;
    rememberFailed(site, count);
    local = { site, count };
  }
</script>

{#if src}
  <!-- Sin «referrer»: la página que sirve el logo no se entera de desde dónde se pide. -->
  <img class="site" {src} alt="" draggable="false" referrerpolicy="no-referrer" width={size} height={size} onerror={onError} />
{:else}
  <Icon name={fallback} size={18} />
{/if}

<style>
  .site { object-fit: contain; border-radius: 6px; user-select: none; }
</style>
