<!-- Ajustes → Buscador (§18.6): redirecciones y búsquedas web, y el historial de uso. -->
<script lang="ts">
  import { onMount } from "svelte";
  import type { Redirect, RedirectKind, SearchSettingsApi } from "../api/search";
  import { REDIRECT_KIND_LABEL, URL_PLACEHOLDER, errorText, moveEntry, usage } from "../stores/search";

  let { api }: { api: SearchSettingsApi } = $props();

  let list: Redirect[] = $state([]);
  let history = $state(0);
  let error: string | null = $state(null);
  let busy = $state(false);

  // Formulario: añadir una entrada o, con `editing`, cambiar la que se eligió.
  let word = $state("");
  let url = $state("");
  let kind: RedirectKind = $state("redirect");
  let editing: number | null = $state(null);

  onMount(() => {
    api.redirectsGet().then((l) => (list = Array.isArray(l) ? l : [])).catch((e) => (error = errorText(e)));
    api.historyLen().then((n) => (history = Number(n) || 0)).catch(() => {});
  });

  /** Guarda la lista completa. El núcleo la valida y devuelve la lista limpia; si algo no vale, se enseña el motivo y no cambia nada. */
  async function persist(next: Redirect[]): Promise<boolean> {
    if (busy) return false;
    busy = true;
    try {
      const saved = await api.redirectsSet(next);
      list = Array.isArray(saved) ? saved : next;
      error = null;
      return true;
    } catch (e) {
      error = errorText(e);
      return false;
    } finally {
      busy = false;
    }
  }

  function reset() {
    word = "";
    url = "";
    kind = "redirect";
    editing = null;
  }

  async function submit() {
    const entry: Redirect = { word: word.trim(), url: url.trim(), kind };
    const next = editing === null ? [...list, entry] : list.map((r, i) => (i === editing ? entry : r));
    if (await persist(next)) reset();
  }

  function edit(i: number) {
    ({ word, url, kind } = list[i]);
    editing = i;
    error = null;
  }

  async function remove(i: number) {
    if (await persist(list.filter((_, j) => j !== i)) && editing !== null) reset();
  }

  async function move(i: number, delta: -1 | 1) {
    const next = moveEntry(list, i, delta);
    if (next !== list) {
      if (await persist(next)) reset();
    }
  }

  async function clearHistory() {
    try {
      history = Number(await api.historyClear()) || 0;
      error = null;
    } catch (e) {
      error = errorText(e);
    }
  }
</script>

<section class="search-settings">
  <p class="hint">
    El buscador se abre con <code>ctrl+shift+espacio</code> (se cambia en <code>config.toml</code>, acción <code>toggle_search</code>). Escribe el nombre de una
    aplicación para abrirla, <code>Enter</code> la abre y <code>ctrl+Enter</code> la abre como administrador.
  </p>

  <h2>Redirecciones y búsquedas web</h2>
  <p class="hint">
    Una <b>redirección</b> abre una dirección al escribir su palabra (<code>claude</code> → claude.com). Una <b>búsqueda web</b> lleva <code>{"{q}"}</code> en la
    dirección y se usa con la palabra y un texto (<code>g gatos</code>).
  </p>

  <ul class="entries" aria-label="Redirecciones y búsquedas web">
    {#each list as r, i (r.word)}
      <li class:editing={editing === i}>
        <span class="main">
          <code class="word">{usage(r)}</code>
          <span class="kind">{REDIRECT_KIND_LABEL[r.kind]}</span>
          <span class="url" title={r.url}>{r.url}</span>
        </span>
        <span class="actions">
          <button aria-label={`Subir ${r.word}`} disabled={i === 0 || busy} onclick={() => move(i, -1)}>↑</button>
          <button aria-label={`Bajar ${r.word}`} disabled={i === list.length - 1 || busy} onclick={() => move(i, 1)}>↓</button>
          <button aria-label={`Editar ${r.word}`} disabled={busy} onclick={() => edit(i)}>Editar</button>
          <button aria-label={`Quitar ${r.word}`} disabled={busy} onclick={() => remove(i)}>Quitar</button>
        </span>
      </li>
    {:else}
      <li class="empty hint">Todavía no hay ninguna. Añade la primera abajo.</li>
    {/each}
  </ul>

  <form
    class="add"
    onsubmit={(e) => {
      e.preventDefault();
      void submit();
    }}
  >
    <input bind:value={word} maxlength="40" placeholder="Palabra (claude, g…)" aria-label="Palabra" autocomplete="off" spellcheck="false" />
    <select bind:value={kind} aria-label="Tipo">
      <option value="redirect">{REDIRECT_KIND_LABEL.redirect}</option>
      <option value="search">{REDIRECT_KIND_LABEL.search}</option>
    </select>
    <input class="url-input" bind:value={url} placeholder={URL_PLACEHOLDER[kind]} aria-label="Dirección" autocomplete="off" spellcheck="false" />
    <button type="submit" disabled={busy || !word.trim() || !url.trim()}>{editing === null ? "Añadir" : "Guardar cambios"}</button>
    {#if editing !== null}<button type="button" onclick={reset}>Cancelar</button>{/if}
  </form>
  {#if error}<p class="error" role="alert">{error}</p>{/if}

  <h2>Historial de uso</h2>
  <p class="hint">
    Lo que más abres sale primero. Se guarda solo en este equipo y no se envía a ningún sitio.
    {history > 0 ? `Ahora recuerda ${history} ${history === 1 ? "elemento" : "elementos"}.` : "Ahora no recuerda nada."}
  </p>
  <button disabled={history === 0} onclick={clearHistory}>Borrar historial</button>
</section>

<style>
  .search-settings { display: flex; flex-direction: column; gap: 8px; }
  .search-settings h2 { margin-top: 10px; }
  .entries { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 5px; max-height: 210px; overflow: auto; scrollbar-width: thin; scrollbar-color: rgba(255, 255, 255, 0.2) transparent; }
  .entries li { display: flex; align-items: center; gap: 8px; padding: 6px 10px; border-radius: 12px; background: rgba(255, 255, 255, 0.05); border: 1px solid var(--line); }
  .entries li.editing { border-color: var(--accent); }
  .entries li.empty { justify-content: center; background: transparent; border: none; }
  .main { display: flex; align-items: baseline; flex-wrap: wrap; gap: 4px 10px; flex: 1; min-width: 0; }
  .word { font-size: 13px; color: var(--fg); }
  .kind { font: 10px var(--mono); letter-spacing: 0.06em; text-transform: uppercase; color: var(--muted); }
  .url { flex-basis: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font: 11px var(--mono); color: var(--muted); }
  .actions { display: flex; gap: 4px; flex: none; }
  .actions :global(button) { padding: 3px 9px !important; }
  .add { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
  .add input { flex: 1 1 140px; }
  .add .url-input { flex: 3 1 240px; }
</style>
