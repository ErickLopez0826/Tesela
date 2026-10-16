<!-- Notas rápidas: escribir, pegar, copiar, editar, borrar, buscar y exportar a Markdown o texto plano. -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { onPanelClosing } from "../../api/state";
  import type { Note, NotesApi } from "../../api/system";
  import { exportContent, exportName, filterNotes, formatStamp, type ExportFormat } from "../../stores/notes";

  let { api }: { api: NotesApi } = $props();

  let notes: Note[] = $state([]);
  let query = $state("");
  let draft = $state("");
  let editing: string | null = $state(null);
  let editText = $state("");
  let confirming: string | null = $state(null);
  let message: string | null = $state(null);
  let error: string | null = $state(null);
  let copiedId: string | null = $state(null);

  const shown = $derived(filterNotes(notes, query));

  function fail(e: unknown) {
    error = String((e as { message?: string })?.message ?? e);
    message = null;
  }

  function ok(text: string) {
    message = text;
    error = null;
  }

  async function load() {
    try {
      notes = await api.list();
    } catch (e) {
      fail(e);
    }
  }

  onMount(() => {
    void load();
    // Lo que se escribió y no se guardó se guarda solo al irse: al ocultarse el panel, al perder el foco la ventana o
    // (más abajo) al pasar a otra pestaña.
    const hidden = () => document.visibilityState === "hidden" && flush();
    document.addEventListener("visibilitychange", hidden);
    window.addEventListener("blur", flush);
    let unlisten: (() => void) | undefined;
    try {
      onPanelClosing(flush).then((u) => (unlisten = u)).catch(() => {});
    } catch {
      /* fuera de Tauri (pruebas) no hay eventos */
    }
    return () => {
      document.removeEventListener("visibilitychange", hidden);
      window.removeEventListener("blur", flush);
      unlisten?.();
    };
  });

  /// Guarda ya, sin esperar, lo que haya a medio escribir: la nota nueva y la nota que se está editando.
  function flush() {
    const text = draft;
    if (text.trim()) {
      draft = "";
      api.add(text).then((note) => (notes = [note, ...notes])).catch((e) => {
        draft = draft ? draft : text; // no se pierde si falla
        fail(e);
      });
    }
    const id = editing;
    const original = id ? notes.find((n) => n.id === id)?.text : undefined;
    if (id && editText.trim() && editText !== original) {
      const edited = editText;
      editing = null;
      api.update(id, edited).then((updated) => (notes = [updated, ...notes.filter((n) => n.id !== updated.id)])).catch(fail);
    }
  }

  // Al pasar a otra pestaña este componente se destruye: lo pendiente se guarda antes.
  onDestroy(flush);

  async function add() {
    if (!draft.trim()) return;
    try {
      const note = await api.add(draft);
      notes = [note, ...notes];
      draft = "";
      ok("Nota guardada.");
    } catch (e) {
      fail(e);
    }
  }

  async function paste() {
    try {
      const text = await api.clipboardRead();
      if (text === null) return ok("El portapapeles no tiene texto.");
      draft = draft ? `${draft}${draft.endsWith("\n") ? "" : "\n"}${text}` : text;
      ok("Pegado.");
    } catch (e) {
      fail(e);
    }
  }

  async function copy(n: Note) {
    try {
      await api.clipboardWrite(n.text);
      copiedId = n.id;
      ok("Copiada.");
      setTimeout(() => copiedId === n.id && (copiedId = null), 1500);
    } catch (e) {
      fail(e);
    }
  }

  function startEdit(n: Note) {
    editing = n.id;
    editText = n.text;
    confirming = null;
  }

  async function saveEdit() {
    if (!editing) return;
    try {
      const updated = await api.update(editing, editText);
      notes = [updated, ...notes.filter((n) => n.id !== updated.id)];
      editing = null;
      ok("Nota actualizada.");
    } catch (e) {
      fail(e);
    }
  }

  async function remove(n: Note) {
    if (confirming !== n.id) {
      confirming = n.id;
      return;
    }
    confirming = null;
    try {
      await api.remove(n.id);
      notes = notes.filter((x) => x.id !== n.id);
      ok("Nota borrada.");
    } catch (e) {
      fail(e);
    }
  }

  async function exportAs(format: ExportFormat) {
    if (shown.length === 0) return ok("No hay notas que exportar.");
    try {
      const path = await api.exportFile(exportName(format), exportContent(shown, format));
      ok(`Guardado en ${path}`);
    } catch (e) {
      fail(e);
    }
  }

  async function copyAll() {
    if (shown.length === 0) return;
    try {
      await api.clipboardWrite(exportContent(shown, "md"));
      ok(`${shown.length} ${shown.length === 1 ? "nota copiada" : "notas copiadas"} como Markdown.`);
    } catch (e) {
      fail(e);
    }
  }
</script>

<section class="section notes">
  <h3>Notas rápidas <span class="count">{notes.length}</span></h3>

  <div class="compose">
    <textarea
      bind:value={draft}
      rows="2"
      placeholder="Escribe una nota…  (Ctrl+Enter para guardarla)"
      aria-label="Nueva nota"
      onkeydown={(e) => e.key === "Enter" && (e.ctrlKey || e.metaKey) && (e.preventDefault(), add())}
    ></textarea>
    <div class="compose-buttons">
      <button class="btn" onclick={paste}>Pegar</button>
      <button class="btn primary" onclick={add} disabled={!draft.trim()}>Agregar nota</button>
    </div>
  </div>

  <input class="search" type="search" bind:value={query} placeholder="Buscar en las notas…" aria-label="Buscar notas" />

  <ul class="list">
    {#each shown as n (n.id)}
      <li class="note">
        {#if editing === n.id}
          <textarea class="edit" bind:value={editText} rows="3" aria-label="Editar nota"></textarea>
          <div class="row">
            <button class="btn primary" onclick={saveEdit} disabled={!editText.trim()}>Guardar</button>
            <button class="btn" onclick={() => (editing = null)}>Cancelar</button>
          </div>
        {:else}
          <div class="text">{n.text}</div>
          <div class="row">
            <small>{formatStamp(n.created)}{n.updated !== n.created ? " · editada" : ""}</small>
            <span class="spacer"></span>
            <button class="btn mini" onclick={() => copy(n)}>{copiedId === n.id ? "Copiada" : "Copiar"}</button>
            <button class="btn mini" onclick={() => startEdit(n)}>Editar</button>
            <button class="btn mini danger" onclick={() => remove(n)} onblur={() => confirming === n.id && (confirming = null)}>
              {confirming === n.id ? "¿Seguro?" : "Borrar"}
            </button>
          </div>
        {/if}
      </li>
    {:else}
      <li class="hint">{notes.length === 0 ? "Aún no hay notas. Escribe la primera arriba." : "Ninguna nota coincide con la búsqueda."}</li>
    {/each}
  </ul>

  <div class="footer">
    <span class="hint">{query.trim() ? `Se exportan las ${shown.length} encontradas` : "Se exportan todas"}</span>
    <span class="spacer"></span>
    <button class="btn mini" onclick={() => exportAs("md")}>Exportar .md</button>
    <button class="btn mini" onclick={() => exportAs("txt")}>Exportar .txt</button>
    <button class="btn mini" onclick={copyAll}>Copiar todo</button>
    <button class="btn mini" onclick={() => api.openFolder().catch(fail)}>Abrir carpeta</button>
  </div>
  {#if message}<div class="notice">{message}</div>{/if}
  {#if error}<div class="notice error">{error}</div>{/if}
</section>

<style>
  .notes { display: flex; flex-direction: column; gap: 8px; grid-column: 1 / -1; }
  .count { margin-left: auto; font-weight: 400; letter-spacing: 0.06em; text-transform: none; }
  h3 { display: flex; }
  .compose { display: flex; gap: 8px; align-items: stretch; }
  .compose textarea { flex: 1; }
  .compose-buttons { display: flex; flex-direction: column; gap: 6px; justify-content: space-between; }
  textarea, .search { font: 13px/1.4 "Segoe UI", system-ui, sans-serif; color: var(--fg); background: rgba(0, 0, 0, 0.5); border: 1px solid var(--line); border-radius: 12px; padding: 8px 10px; resize: none; box-sizing: border-box; width: 100%; }
  /* Al editar, un 20 % más alto (no más ancho): 3 filas son unos 73 px. */
  textarea.edit { height: 88px; }
  textarea:focus, .search:focus { outline: none; border-color: var(--accent); }
  .list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; max-height: 230px; overflow: auto; scrollbar-width: thin; scrollbar-color: rgba(255, 255, 255, 0.2) transparent; }
  .note { padding: 8px 10px; border-radius: 14px; background: rgba(255, 255, 255, 0.05); border: 1px solid var(--line); display: flex; flex-direction: column; gap: 6px; }
  .text { white-space: pre-wrap; overflow-wrap: anywhere; font-size: 13px; max-height: 5.4em; overflow: hidden; }
  .row, .footer { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
  .spacer { flex: 1; }
  small { color: var(--muted); font: 10px var(--mono); }
  .mini { padding: 3px 10px; font-size: 10px; }
  .footer { padding-top: 6px; border-top: 1px dotted var(--line); }
  .btn.danger { background: transparent; }
</style>
