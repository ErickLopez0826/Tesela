<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import type { ActionOutcome, WallpaperList } from "../../api/system";

  let {
    list,
    onSet,
  }: {
    list: WallpaperList | null;
    onSet: (name: string) => Promise<ActionOutcome>;
  } = $props();

  let outcome: ActionOutcome | null = $state(null);
  let error: string | null = $state(null);
  // Imagen en el centro del carrusel; por defecto la que está puesta ahora.
  let picked: number | null = $state(null);

  const images = $derived(list?.images ?? []);
  const center = $derived(picked ?? Math.max(0, images.findIndex((n) => n === list?.current)));

  function src(name: string): string {
    const sep = list?.dir.includes("\\") ? "\\" : "/";
    try {
      return convertFileSrc(`${list?.dir}${sep}${name}`);
    } catch {
      return ""; // fuera de Tauri (pruebas) no hay protocolo de archivos
    }
  }

  async function apply(name: string) {
    error = null;
    try {
      outcome = await onSet(name);
    } catch (e) {
      outcome = null;
      error = String(e);
    }
  }

  // Un clic en una miniatura lateral la lleva al centro; un clic en la central la aplica.
  function choose(i: number) {
    if (i === center) void apply(images[i]);
    else picked = i;
  }
</script>

<section class="section">
  <h3>Fondos de pantalla <span class="count">{images.length} {images.length === 1 ? "imagen" : "imágenes"}</span></h3>
  {#if images.length > 0}
    <div class="carousel" role="listbox" aria-label="Fondos de pantalla">
      {#each images as name, i (name)}
        {@const d = Math.abs(i - center)}
        {#if d <= 3}
          <button
            class="thumb"
            class:center={d === 0}
            class:current={name === list?.current}
            style:--d={d}
            style:--side={i < center ? -1 : 1}
            title={d === 0 ? `Aplicar ${name}` : name}
            aria-label={name}
            role="option"
            aria-selected={d === 0}
            onclick={() => choose(i)}
          >
            <img src={src(name)} alt="" loading="lazy" />
          </button>
        {/if}
      {/each}
    </div>
    <div class="hint">{images[center]}{images[center] === list?.current ? " · puesto ahora" : " · clic para aplicar"}</div>
  {:else}
    <div class="hint">Copia imágenes (jpg, png, bmp, webp) en <code>{list?.dir ?? "la carpeta de fondos"}</code> y aparecerán aquí.</div>
  {/if}
  {#if outcome}<div class="notice">{outcome.description}</div>{/if}
  {#if error}<div class="notice error">{error}</div>{/if}
</section>

<style>
  .count { margin-left: auto; letter-spacing: 0.06em; text-transform: none; font-weight: 400; }
  h3 { display: flex; }
  .carousel { display: flex; align-items: center; justify-content: center; gap: 8px; height: 150px; margin: 4px 0 8px; perspective: 600px; }
  .thumb { padding: 0; flex: none; width: calc(70px - var(--d) * 8px); height: calc(110px - var(--d) * 14px); overflow: hidden; border-radius: 16px; border: 1px solid var(--line); background: #111; cursor: pointer; transform: skewX(-7deg); opacity: calc(1 - var(--d) * 0.22); transition: width 0.5s cubic-bezier(.3, 1.1, .4, 1), height 0.5s cubic-bezier(.3, 1.1, .4, 1), opacity 0.4s, box-shadow 0.4s, border-color 0.4s; }
  .thumb.center { width: 230px; height: 140px; opacity: 1; border-color: rgba(255, 255, 255, 0.5); box-shadow: 0 10px 30px -10px rgba(0, 0, 0, 0.8); }
  .thumb.current { border-color: var(--accent); box-shadow: 0 0 18px -6px rgba(255, 255, 255, 0.8); }
  .thumb:hover { border-color: #fff; }
  .thumb img { width: 140%; height: 100%; margin-left: -20%; object-fit: cover; transform: skewX(7deg); display: block; }
</style>
