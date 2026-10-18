<!-- Lo que hay detrás de la barra o del panel, desenfocado con la intensidad de Personalización. Va bajo el tinte. -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { onWallpaperChanged } from "../stores/theme";

  let { kind, blur, tick = 0 }: { kind: "bar" | "panel"; blur: number; tick?: number } = $props();

  interface Glass {
    url: string;
    width: number;
    height: number;
  }

  /** La barra vuelve a copiar el fondo cada tanto por si Wallpaper Engine lo cambió por su cuenta. */
  const REFRESH_EVERY_MS = 120_000;
  const SLIDER_DELAY_MS = 160;

  let image: Glass | null = $state(null);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let seq = 0;

  async function load(refresh: boolean) {
    const mine = ++seq;
    if (blur <= 0) {
      image = null;
      return;
    }
    try {
      const glass = await invoke<Glass | null>("glass_image", { kind, blur, refresh });
      if (mine === seq) image = glass;
    } catch {
      if (mine === seq) image = null;
    }
  }

  // Al cambiar el desenfoque (se espera a que el deslizador se detenga) y, en el panel, cada vez que se abre.
  $effect(() => {
    const amount = blur;
    void tick;
    clearTimeout(timer);
    timer = setTimeout(() => void load(false), kind === "panel" ? 0 : SLIDER_DELAY_MS);
    return () => clearTimeout(timer);
  });

  onMount(() => {
    if (kind !== "bar") return;
    const stopWallpaper = onWallpaperChanged(() => void load(true));
    const every = setInterval(() => document.visibilityState !== "hidden" && void load(true), REFRESH_EVERY_MS);
    return () => {
      stopWallpaper();
      clearInterval(every);
    };
  });

  const scale = typeof window === "undefined" ? 1 : window.devicePixelRatio || 1;
</script>

{#if image}
  <img class="glass" src={image.url} alt="" draggable="false" style:width={`${image.width / scale}px`} style:height={`${image.height / scale}px`} />
{/if}

<style>
  .glass { position: absolute; left: 0; top: 0; z-index: 0; max-width: none; pointer-events: none; user-select: none; animation: glass-in 0.35s ease both; }
  @keyframes glass-in { from { opacity: 0; } }
  @media (prefers-reduced-motion: reduce) { .glass { animation: none; } }
</style>
