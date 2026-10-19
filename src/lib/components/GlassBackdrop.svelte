<!-- Lo que hay detrás de la barra o del panel, desenfocado con la intensidad de Personalización. Va bajo el tinte. -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { imageLuma } from "../stores/tone";
  import { onWallpaperChanged } from "../stores/theme";

  /** `onLuma`: avisa de la luminosidad (0 a 1) de lo que hay detrás, para elegir un texto que se lea (aunque no haya desenfoque). */
  let {
    kind,
    blur,
    tick = 0,
    onLuma,
    anchored = null,
  }: {
    kind: "bar" | "panel";
    blur: number;
    tick?: number;
    onLuma?: (luma: number | null) => void;
    /** Isla de la barra de tareas: la imagen cubre la zona máxima y se coloca según el tamaño actual (centrada, pegada abajo). */
    anchored?: { gap: number } | null;
  } = $props();

  /** Desenfoque con el que se pide la copia cuando solo hace falta para medir su luminosidad. */
  const TONE_BLUR = 20;

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
    const visible = blur > 0;
    if (!visible && !onLuma) {
      image = null;
      return;
    }
    try {
      const glass = await invoke<Glass | null>("glass_image", { kind, blur: visible ? blur : TONE_BLUR, refresh });
      if (mine !== seq) return;
      image = visible ? glass : null;
      if (onLuma) void imageLuma(glass?.url).then((l) => mine === seq && onLuma(l));
    } catch {
      if (mine !== seq) return;
      image = null;
      onLuma?.(null);
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
    const stopWallpaper = onWallpaperChanged(() => void load(true));
    // El panel abierto también sigue al fondo; la barra además lo vuelve a copiar cada tanto por si cambió por su cuenta.
    const every = kind === "bar" ? setInterval(() => document.visibilityState !== "hidden" && void load(true), REFRESH_EVERY_MS) : undefined;
    return () => {
      stopWallpaper();
      if (every) clearInterval(every);
    };
  });

  const scale = typeof window === "undefined" ? 1 : window.devicePixelRatio || 1;
</script>

{#if image}
  <img
    class="glass"
    class:anchored={anchored !== null}
    src={image.url}
    alt=""
    draggable="false"
    style:width={`${image.width / scale}px`}
    style:height={`${image.height / scale}px`}
    style:--iw={`${image.width / scale}px`}
    style:--ih={`${image.height / scale}px`}
    style:--gap={anchored ? `${anchored.gap}px` : undefined}
  />
{/if}

<style>
  .glass { position: absolute; left: 0; top: 0; z-index: 0; max-width: none; pointer-events: none; user-select: none; animation: glass-in 0.35s ease both; }
  /* Sobre la isla: el borde de abajo de la isla queda `gap` por encima del de la imagen y el centro coincide; sigue su tamaño. */
  .glass.anchored { left: calc((100% - var(--iw)) / 2); top: calc(100% + var(--gap) - var(--ih)); }
  @keyframes glass-in { from { opacity: 0; } }
  @media (prefers-reduced-motion: reduce) { .glass { animation: none; } }
</style>
