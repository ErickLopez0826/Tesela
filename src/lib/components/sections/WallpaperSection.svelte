<script lang="ts">
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

  async function apply(name: string) {
    error = null;
    try {
      outcome = await onSet(name);
    } catch (e) {
      outcome = null;
      error = String(e);
    }
  }
</script>

<section class="section">
  <h3>Fondos de pantalla</h3>
  {#if list && list.images.length > 0}
    <div class="buttons">
      {#each list.images as name (name)}
        <button class="btn" onclick={() => apply(name)}>{name}</button>
      {/each}
    </div>
  {:else}
    <div class="hint">Copia imágenes (jpg, png, bmp, webp) en <code>{list?.dir ?? "la carpeta de fondos"}</code> y aparecerán aquí.</div>
  {/if}
  {#if outcome}<div class="notice">{outcome.description}</div>{/if}
  {#if error}<div class="notice error">{error}</div>{/if}
</section>
