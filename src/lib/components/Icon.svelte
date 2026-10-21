<!-- Iconos de línea (trazo 2, estilo Lucide) en SVG: sustituyen a los emojis. Heredan el color del texto. -->
<script lang="ts">
  export type IconName =
    | "volume"
    | "volume-off"
    | "play"
    | "pause"
    | "previous"
    | "next"
    | "music"
    | "arrow-down"
    | "arrow-up"
    | "wifi"
    | "bluetooth"
    | "share"
    | "moon"
    | "refresh"
    | "power"
    | "snowflake"
    | "folder"
    | "search"
    | "globe"
    | "ethernet";
  /** `level`: solo el Wi-Fi. Cuántos de sus 3 arcos se ven encendidos (0 a 3); los demás quedan tenues. */
  let { name, size = 16, level = 3 }: { name: IconName; size?: number; level?: number } = $props();
</script>

<svg class="icon" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  {#if name === "volume"}
    <path d="M11 5 6 9H2v6h4l5 4V5z" /><path d="M15.5 8.5a5 5 0 0 1 0 7" /><path d="M19 5a10 10 0 0 1 0 14" />
  {:else if name === "volume-off"}
    <path d="M11 5 6 9H2v6h4l5 4V5z" /><path d="m22 9-6 6" /><path d="m16 9 6 6" />
  {:else if name === "play"}
    <path d="M6 4v16l14-8L6 4z" fill="currentColor" />
  {:else if name === "pause"}
    <rect x="6" y="4" width="4" height="16" rx="1" fill="currentColor" /><rect x="14" y="4" width="4" height="16" rx="1" fill="currentColor" />
  {:else if name === "previous"}
    <path d="M19 20 9 12l10-8v16z" fill="currentColor" /><path d="M5 19V5" />
  {:else if name === "next"}
    <path d="m5 4 10 8-10 8V4z" fill="currentColor" /><path d="M19 5v14" />
  {:else if name === "music"}
    <path d="M9 18V5l12-2v13" /><circle cx="6" cy="18" r="3" /><circle cx="18" cy="16" r="3" />
  {:else if name === "arrow-down"}
    <path d="M12 5v14" /><path d="m19 12-7 7-7-7" />
  {:else if name === "arrow-up"}
    <path d="M12 19V5" /><path d="m5 12 7-7 7 7" />
  {:else if name === "wifi"}
    <path class="arc" class:dim={level < 3} d="M1.42 9a16 16 0 0 1 21.16 0" />
    <path class="arc" class:dim={level < 2} d="M5 12.55a11 11 0 0 1 14.08 0" />
    <path class="arc" class:dim={level < 1} d="M8.53 16.11a6 6 0 0 1 6.95 0" />
    <path d="M12 20h.01" />
  {:else if name === "bluetooth"}
    <path d="m7 7 10 10-5 5V2l5 5L7 17" />
  {:else if name === "share"}
    <circle cx="18" cy="5" r="3" /><circle cx="6" cy="12" r="3" /><circle cx="18" cy="19" r="3" /><path d="m8.59 13.51 6.83 3.98" /><path d="m15.41 6.51-6.82 3.98" />
  {:else if name === "moon"}
    <path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9z" />
  {:else if name === "refresh"}
    <path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8" /><path d="M21 3v5h-5" /><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16" /><path d="M8 16H3v5" />
  {:else if name === "power"}
    <path d="M12 2v10" /><path d="M18.4 6.6a9 9 0 1 1-12.77.04" />
  {:else if name === "snowflake"}
    <path d="M2 12h20" /><path d="M12 2v20" /><path d="m20 16-4-4 4-4" /><path d="m4 8 4 4-4 4" /><path d="m16 4-4 4-4-4" /><path d="m8 20 4-4 4 4" />
  {:else if name === "folder"}
    <path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" />
  {:else if name === "search"}
    <circle cx="11" cy="11" r="8" /><path d="m21 21-4.3-4.3" />
  {:else if name === "ethernet"}
    <!-- Conector RJ45 con su cable: la conexión por cable. -->
    <path d="M6 3h12v9a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2z" /><path d="M9 3v4" /><path d="M12 3v4" /><path d="M15 3v4" /><path d="M10 14v3a2 2 0 0 0 2 2 2 2 0 0 1 2 2v1" />
  {:else if name === "globe"}
    <circle cx="12" cy="12" r="10" /><path d="M2 12h20" /><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
  {/if}
</svg>

<style>
  .icon { display: inline-block; vertical-align: -0.2em; flex: none; }
  /* Wi-Fi: los arcos que la señal no alcanza quedan tenues y cambian suavemente al subir o bajar. */
  .arc { transition: opacity 0.4s ease; }
  .arc.dim { opacity: 0.25; }
</style>
