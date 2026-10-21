<!-- Wallpaper Engine no se encontró en la ruta de siempre: pide la carpeta donde está instalado y dice dónde suele estar. -->
<script lang="ts">
  import { errorText } from "../../stores/search";

  let { onSave }: { onSave: (path: string) => Promise<unknown> } = $props();

  const EXAMPLE = String.raw`C:\Program Files (x86)\Steam\steamapps\common\wallpaper_engine`;

  let path = $state("");
  let busy = $state(false);
  let error: string | null = $state(null);

  async function save() {
    if (busy) return;
    busy = true;
    error = null;
    try {
      await onSave(path);
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }
</script>

<section class="section engine-path">
  <h3>Wallpaper Engine</h3>
  <p class="hint">
    No lo encuentro. Si lo tienes instalado, indica su carpeta (la que contiene <code>wallpaper64.exe</code>). Con Steam suele estar en
    <code>{EXAMPLE}</code>; para verla: Steam → Wallpaper Engine → clic derecho → Administrar → Explorar archivos locales. Si tu biblioteca de
    Steam está en otro disco, será algo como <code>D:\SteamLibrary\steamapps\common\wallpaper_engine</code>.
  </p>
  <form onsubmit={(e) => (e.preventDefault(), void save())}>
    <input type="text" bind:value={path} placeholder={EXAMPLE} aria-label="Carpeta de Wallpaper Engine" spellcheck="false" />
    <button type="submit" disabled={busy || path.trim() === ""}>{busy ? "Comprobando…" : "Usar esta carpeta"}</button>
  </form>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</section>

<style>
  .hint { margin: 0 0 8px; color: var(--muted); font-size: 12px; line-height: 1.45; }
  code { word-break: break-all; }
  form { display: flex; gap: 6px; }
  input { flex: 1; min-width: 0; }
  .error { margin: 6px 0 0; font-size: 12px; }
</style>
