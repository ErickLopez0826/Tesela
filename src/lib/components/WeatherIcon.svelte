<!-- Icono del tiempo (trazo 2, estilo Lucide): sol, luna, nube, nube con sol, lluvia, nieve, tormenta o niebla. -->
<script lang="ts">
  import { weatherKind } from "../stores/weather";

  let { code, night = false, size = 30 }: { code: number; night?: boolean; size?: number } = $props();

  const kind = $derived(weatherKind(code, night));
</script>

<svg class="weather-icon" data-kind={kind} width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  {#if kind === "sun"}
    <circle cx="12" cy="12" r="4" /><path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41" />
  {:else if kind === "moon"}
    <path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z" />
  {:else if kind === "partly-sun"}
    <path d="M12 2v2M4.93 4.93l1.41 1.41M20 12h2M19.07 4.93l-1.41 1.41" /><path d="M15.947 12.65a4 4 0 0 0-5.925-4.128" /><path d="M13 22H7a5 5 0 1 1 4.9-6H13a3 3 0 0 1 0 6Z" />
  {:else if kind === "partly-moon"}
    <path d="M13 16a3 3 0 0 1 0 6H7a5 5 0 1 1 4.9-6Z" /><path d="M18.376 14.512a6 6 0 0 0 3.461-4.127c.148-.625-.659-.97-1.248-.714a4 4 0 0 1-5.259-5.26c.255-.589-.09-1.395-.716-1.248a6 6 0 0 0-4.594 5.36" />
  {:else if kind === "cloud"}
    <path d="M17.5 19H9a7 7 0 1 1 6.71-9h1.79a4.5 4.5 0 1 1 0 9Z" />
  {:else if kind === "fog"}
    <path d="M4 14.899A7 7 0 1 1 15.71 8h1.79a4.5 4.5 0 0 1 2.5 8.242" /><path d="M16 17H7M17 21H9" />
  {:else if kind === "drizzle"}
    <path d="M4 14.899A7 7 0 1 1 15.71 8h1.79a4.5 4.5 0 0 1 2.5 8.242" /><path d="M8 19v1M8 14v1M16 19v1M16 14v1M12 21v1M12 16v1" />
  {:else if kind === "rain"}
    <path d="M4 14.899A7 7 0 1 1 15.71 8h1.79a4.5 4.5 0 0 1 2.5 8.242" /><path d="M16 14v6M8 14v6M12 16v6" />
  {:else if kind === "snow"}
    <path d="M4 14.899A7 7 0 1 1 15.71 8h1.79a4.5 4.5 0 0 1 2.5 8.242" /><path d="M8 15h.01M8 19h.01M12 17h.01M12 21h.01M16 15h.01M16 19h.01" />
  {:else if kind === "storm"}
    <path d="M6 16.326A7 7 0 1 1 15.71 8h1.79a4.5 4.5 0 0 1 .5 8.973" /><path d="m13 12-3 5h4l-3 5" />
  {:else}
    <path d="M17.5 19H9a7 7 0 1 1 6.71-9h1.79a4.5 4.5 0 1 1 0 9Z" /><path d="M12 13v.01M12 10a1.7 1.7 0 0 1 1.2 2.9c-.6.5-1.2.8-1.2 1.6" />
  {/if}
</svg>

<style>
  .weather-icon { color: var(--accent); filter: drop-shadow(0 0 8px color-mix(in srgb, var(--accent) 45%, transparent)); flex: none; }
</style>
