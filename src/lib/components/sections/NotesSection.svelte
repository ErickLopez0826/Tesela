<!-- Notas rápidas: escribir, pegar, copiar, editar, borrar, buscar (por texto y por fecha) y exportar una, varias o todas. -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import DateField from "../DateField.svelte";
  import { onPanelClosing } from "../../api/state";
  import type { Note, NotesApi } from "../../api/system";
  import {
    RANGE_PRESETS,
    describeRange,
    exportContent,
    exportFileName,
    filterNotes,
    formatStamp,
    presetRange,
    type ExportFormat,
    type RangePreset,
  } from "../../stores/notes";

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
  let from = $state("");
  let to = $state("");
  let selected: string[] = $state([]);
  /** Notas que se están borrando: se desvanecen y colapsan antes de quitarse de la lista. */
  let leaving: string[] = $state([]);
  const LEAVE_MS = 380;

  const shown = $derived(filterNotes(notes, query, { from, to }));
  const filtering = $derived(query.trim() !== "" || from !== "" || to !== "");
  // Lo que se exporta (y se copia): las notas marcadas, sean las que sean; si no hay ninguna marcada, todas las que se ven.
  const marked = $derived(notes.filter((n) => selected.includes(n.id)));
  const target = $derived(marked.length > 0 ? marked : shown);
  const allShownMarked = $derived(shown.length > 0 && shown.every((n) => selected.includes(n.id)));

  let datesOpen = $state(false);
  const hasDates = $derived(from !== "" || to !== "");
  const isPreset = (id: RangePreset) => {
    const r = presetRange(id);
    return from === r.from && to === r.to;
  };
  // Tocar el atajo que ya está puesto lo quita.
  const pickPreset = (id: RangePreset) => {
    if (isPreset(id)) return clearDates();
    ({ from, to } = presetRange(id));
  };
  const clearDates = () => {
    from = "";
    to = "";
  };

  const toggle = (id: string) => (selected = selected.includes(id) ? selected.filter((x) => x !== id) : [...selected, id]);
  const toggleShown = () => {
    const ids = shown.map((n) => n.id);
    selected = allShownMarked ? selected.filter((id) => !ids.includes(id)) : [...new Set([...selected, ...ids])];
  };

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
    if (leaving.includes(n.id)) return;
    leaving = [...leaving, n.id];
    const reduced = typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
    const animation = new Promise<void>((done) => setTimeout(done, reduced ? 0 : LEAVE_MS));
    try {
      await api.remove(n.id);
      await animation;
      notes = notes.filter((x) => x.id !== n.id);
      selected = selected.filter((id) => id !== n.id);
      ok("Nota borrada.");
    } catch (e) {
      fail(e);
    } finally {
      leaving = leaving.filter((id) => id !== n.id);
    }
  }

  async function exportAs(format: ExportFormat) {
    const notesToExport = target;
    if (notesToExport.length === 0) return ok("No hay notas que exportar.");
    try {
      const path = await api.exportFile(exportFileName(notesToExport, format), exportContent(notesToExport, format));
      ok(`${notesToExport.length === 1 ? "Nota guardada" : `${notesToExport.length} notas guardadas`} en ${path}`);
    } catch (e) {
      fail(e);
    }
  }

  async function copyTarget() {
    const notesToCopy = target;
    if (notesToCopy.length === 0) return;
    try {
      await api.clipboardWrite(exportContent(notesToCopy, "md"));
      ok(`${notesToCopy.length} ${notesToCopy.length === 1 ? "nota copiada" : "notas copiadas"} como Markdown.`);
    } catch (e) {
      fail(e);
    }
  }

  const exportHint = $derived(
    marked.length > 0
      ? `Se ${marked.length === 1 ? "exporta la nota seleccionada" : `exportan las ${marked.length} seleccionadas`}`
      : filtering
        ? `Se exportan las ${shown.length} encontradas`
        : "Se exportan todas",
  );
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

  <div class="finder">
    <input class="search" type="search" bind:value={query} placeholder="Buscar en las notas…" aria-label="Buscar notas" />
    <button
      class="filter-toggle"
      class:active={hasDates}
      aria-expanded={datesOpen}
      aria-controls="notes-dates"
      aria-label="Filtrar por fecha"
      onclick={() => (datesOpen = !datesOpen)}
    >
      <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <rect x="3.5" y="5" width="17" height="15.5" rx="3" /><path d="M3.5 10h17M8 3v4M16 3v4" />
      </svg>
      Fecha
      {#if hasDates}<i class="dot" aria-hidden="true"></i>{/if}
    </button>
  </div>

  {#if datesOpen || hasDates}
    <div class="dates" id="notes-dates">
      <div class="chips" role="group" aria-label="Atajos de fecha">
        {#each RANGE_PRESETS as p (p.id)}
          <button class="chip" class:on={isPreset(p.id)} aria-pressed={isPreset(p.id)} onclick={() => pickPreset(p.id)}>{p.label}</button>
        {/each}
      </div>
      <div class="range">
        <div class="date-field">
          <span>Desde</span>
          <DateField bind:value={from} label="Desde la fecha" max={to || undefined} />
        </div>
        <span class="arrow" aria-hidden="true">→</span>
        <div class="date-field">
          <span>Hasta</span>
          <DateField bind:value={to} label="Hasta la fecha" min={from || undefined} />
        </div>
      </div>
      {#if hasDates}
        <div class="summary">
          <span>{describeRange({ from, to })} · {shown.length} {shown.length === 1 ? "nota" : "notas"}</span>
          <button class="btn mini" onclick={clearDates}>Quitar fechas</button>
        </div>
      {/if}
    </div>
  {/if}

  {#if notes.length > 0}
    <div class="pickbar">
      <label class="pick all">
        <input type="checkbox" checked={allShownMarked} disabled={shown.length === 0} onchange={toggleShown} aria-label="Seleccionar las notas que se ven" />
        {filtering ? "Todas las encontradas" : "Todas"}
      </label>
      {#if marked.length > 0}<button class="btn mini" onclick={() => (selected = [])}>Quitar selección ({marked.length})</button>{/if}
    </div>
  {/if}

  <ul class="list">
    {#each shown as n (n.id)}
      <li class="note" class:marked={selected.includes(n.id)} class:leaving={leaving.includes(n.id)}>
       <div class="card">
        {#if editing === n.id}
          <textarea class="edit" bind:value={editText} rows="3" aria-label="Editar nota"></textarea>
          <div class="row">
            <button class="btn primary" onclick={saveEdit} disabled={!editText.trim()}>Guardar</button>
            <button class="btn" onclick={() => (editing = null)}>Cancelar</button>
          </div>
        {:else}
          <div class="text">{n.text}</div>
          <div class="row">
            <input type="checkbox" class="pick" checked={selected.includes(n.id)} onchange={() => toggle(n.id)} aria-label={`Seleccionar nota del ${formatStamp(n.created)}`} />
            <small>{formatStamp(n.created)}{n.updated !== n.created ? " · editada" : ""}</small>
            <span class="spacer"></span>
            <button class="btn mini" onclick={() => copy(n)}>{copiedId === n.id ? "Copiada" : "Copiar"}</button>
            <button class="btn mini" onclick={() => startEdit(n)}>Editar</button>
            <button class="btn mini danger" onclick={() => remove(n)} onblur={() => confirming === n.id && (confirming = null)}>
              {confirming === n.id ? "¿Seguro?" : "Borrar"}
            </button>
          </div>
        {/if}
       </div>
      </li>
    {:else}
      <li class="hint">{notes.length === 0 ? "Aún no hay notas. Escribe la primera arriba." : "Ninguna nota coincide con la búsqueda."}</li>
    {/each}
  </ul>

  <div class="footer">
    <span class="hint">{exportHint}</span>
    <span class="spacer"></span>
    <button class="btn mini" onclick={() => exportAs("md")}>Exportar .md</button>
    <button class="btn mini" onclick={() => exportAs("txt")}>Exportar .txt</button>
    <button class="btn mini" onclick={copyTarget}>{marked.length > 0 ? "Copiar selección" : "Copiar todo"}</button>
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
  .list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; max-height: 230px; overflow: auto; scrollbar-width: thin; scrollbar-color: rgba(255, 255, 255, 0.2) transparent; }
  /* Al borrarla la nota se desvanece, se desliza y colapsa su altura: las de abajo suben suavemente. */
  .note { display: grid; grid-template-rows: 1fr; margin-bottom: 6px; transition: grid-template-rows 0.38s cubic-bezier(.3, 0, .2, 1), margin-bottom 0.38s cubic-bezier(.3, 0, .2, 1), opacity 0.3s ease, transform 0.38s ease; }
  .card { min-height: 0; overflow: hidden; padding: 8px 10px; border-radius: 14px; background: rgba(255, 255, 255, 0.05); border: 1px solid var(--line); display: flex; flex-direction: column; gap: 6px; }
  .note.leaving { grid-template-rows: 0fr; margin-bottom: 0; opacity: 0; transform: translateX(14px); pointer-events: none; }
  .note.leaving .card { padding-top: 0; padding-bottom: 0; border-color: transparent; }
  @media (prefers-reduced-motion: reduce) { .note { transition: none; } }
  .text { white-space: pre-wrap; overflow-wrap: anywhere; font-size: 13px; max-height: 5.4em; overflow: hidden; }
  .row, .footer { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
  .spacer { flex: 1; }
  small { color: var(--muted); font: 10px var(--mono); }
  .mini { padding: 3px 10px; font-size: 10px; }
  .footer { padding-top: 6px; border-top: 1px dotted var(--line); }
  .finder { display: flex; gap: 6px; align-items: stretch; }
  .finder .search { flex: 1; min-width: 0; }
  .filter-toggle {
    position: relative; display: inline-flex; align-items: center; gap: 6px; flex: none; cursor: pointer;
    font: 11px var(--mono); letter-spacing: 0.06em; text-transform: uppercase; color: var(--muted);
    background: rgba(0, 0, 0, 0.5); border: 1px solid var(--line); border-radius: 12px; padding: 0 12px;
    transition: color 0.25s, border-color 0.25s, background 0.25s;
  }
  .filter-toggle:hover, .filter-toggle[aria-expanded="true"] { color: var(--fg); border-color: var(--accent); }
  .filter-toggle.active { color: var(--accent); border-color: var(--accent); }
  .dot { width: 6px; height: 6px; border-radius: 50%; background: var(--accent); box-shadow: 0 0 8px var(--accent); }

  .dates {
    display: flex; flex-direction: column; gap: 8px; padding: 10px; border-radius: 14px;
    background: rgba(255, 255, 255, 0.04); border: 1px solid var(--line); animation: dates-in 0.25s cubic-bezier(.2, .8, .2, 1) both;
  }
  @keyframes dates-in { from { opacity: 0; transform: translateY(-4px); } }
  @media (prefers-reduced-motion: reduce) { .dates { animation: none; } }
  .chips { display: flex; gap: 6px; flex-wrap: wrap; }
  .chip {
    font: 11px var(--mono); letter-spacing: 0.04em; color: var(--muted); cursor: pointer; padding: 5px 12px; border-radius: 999px;
    background: transparent; border: 1px solid var(--line); transition: color 0.25s, background 0.25s, border-color 0.25s, box-shadow 0.25s;
  }
  .chip:hover { color: var(--fg); border-color: var(--accent); }
  .chip.on { color: var(--on-accent); background: var(--accent); border-color: transparent; box-shadow: 0 0 14px -6px var(--accent); }
  .range { display: flex; align-items: flex-end; gap: 8px; }
  .arrow { color: var(--muted); font-size: 14px; flex: none; padding-bottom: 8px; }
  .date-field { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px; }
  .date-field span { font: 10px var(--mono); letter-spacing: 0.08em; text-transform: uppercase; color: var(--muted); padding-left: 4px; }
  .summary { display: flex; align-items: center; justify-content: space-between; gap: 8px; font: 11px var(--mono); color: var(--accent); }
  .pickbar { display: flex; align-items: center; justify-content: space-between; gap: 8px; font: 11px var(--mono); color: var(--muted); }
  .pick { accent-color: var(--accent); cursor: pointer; width: 15px; height: 15px; margin: 0; }
  .pick.all { display: flex; align-items: center; gap: 6px; cursor: pointer; }
  .note.marked .card { border-color: var(--accent); background: rgba(255, 255, 255, 0.09); }
  .btn.danger { background: transparent; }
</style>
