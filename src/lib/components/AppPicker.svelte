<!-- Buscador de aplicaciones: escribes y salen las que tienes instaladas o abiertas; eliges una de la lista. -->
<script lang="ts">
  import { appsApi, type AppSuggestion } from "../api/apps";

  let {
    value = $bindable(""),
    label,
    placeholder = "",
    load = appsApi.suggestions,
    onPick = () => {},
  }: {
    value?: string;
    label: string;
    placeholder?: string;
    /** De dónde salen las aplicaciones (se leen al enfocar el campo por primera vez). */
    load?: () => Promise<AppSuggestion[]>;
    onPick?: (app: AppSuggestion) => void;
  } = $props();

  const MAX_RESULTS = 60;

  let apps: AppSuggestion[] | null = $state(null);
  let loading = $state(false);
  let open = $state(false);
  let active = $state(-1);

  /** Sin mayúsculas ni acentos ni «.exe», para que «excel», «Excel» y «EXCEL.EXE» se encuentren igual. */
  function normalize(text: string): string {
    return text.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase().trim().replace(/\.exe$/, "");
  }

  async function ensureLoaded() {
    if (apps || loading) return;
    loading = true;
    try {
      apps = await load();
    } catch {
      apps = [];
    } finally {
      loading = false;
    }
  }

  const matches = $derived.by(() => {
    const q = normalize(value);
    const all = apps ?? [];
    return (q ? all.filter((a) => normalize(a.name).includes(q) || a.exe.includes(q)) : all).slice(0, MAX_RESULTS);
  });

  function pick(app: AppSuggestion) {
    open = false;
    active = -1;
    onPick(app);
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape" && open) {
      open = false;
      e.stopPropagation();
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      open = true;
      void ensureLoaded();
      const step = e.key === "ArrowDown" ? 1 : -1;
      active = matches.length ? (active + step + matches.length) % matches.length : -1;
    } else if (e.key === "Enter" && open && active >= 0 && matches[active]) {
      // Solo si se eligió con las flechas: si no, Enter sigue enviando lo que se escribió.
      e.preventDefault();
      pick(matches[active]);
    }
  }
</script>

<div class="picker">
  <input
    type="text"
    role="combobox"
    aria-label={label}
    aria-expanded={open}
    aria-autocomplete="list"
    {placeholder}
    autocomplete="off"
    bind:value
    onfocus={() => {
      open = true;
      void ensureLoaded();
    }}
    oninput={() => {
      open = true;
      active = -1;
      void ensureLoaded();
    }}
    onblur={() => setTimeout(() => (open = false), 120)}
    onkeydown={onKey}
  />
  {#if open}
    <ul class="list" role="listbox" aria-label={`Aplicaciones para ${label}`}>
      {#if loading}
        <li class="empty">Buscando tus aplicaciones…</li>
      {:else if matches.length === 0}
        <li class="empty">{value.trim() ? "Ninguna aplicación coincide" : "No se encontraron aplicaciones"}</li>
      {/if}
      {#each matches as app, i (app.path + "|" + app.exe + "|" + app.name)}
        <!-- mousedown (y no click): así el campo no pierde el foco antes de elegir -->
        <li role="option" aria-selected={i === active} class:active={i === active} onmousedown={(e) => (e.preventDefault(), pick(app))}>
          <span class="name">{app.name}</span>
          <span class="exe">{app.exe}.exe</span>
          {#if app.running}<span class="live">abierta</span>{/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .picker { position: relative; flex: 1 1 220px; min-width: 160px; }
  input { width: 100%; box-sizing: border-box; }
  .list {
    position: absolute; left: 0; right: 0; top: calc(100% + 4px); z-index: 20; margin: 0; padding: 4px; list-style: none;
    max-height: 230px; overflow-y: auto; border-radius: 12px; border: 1px solid var(--line, rgba(255, 255, 255, 0.14));
    background: #121214; box-shadow: 0 14px 30px -10px rgba(0, 0, 0, 0.8); scrollbar-width: thin;
  }
  li { display: flex; align-items: baseline; gap: 8px; padding: 6px 10px; border-radius: 8px; cursor: pointer; font: 13px "Segoe UI", system-ui, sans-serif; }
  li.empty { cursor: default; color: var(--muted, #8b8b90); font-size: 12px; }
  li:not(.empty):hover, li.active { background: rgba(255, 255, 255, 0.1); }
  .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .exe { font: 11px var(--mono, monospace); color: var(--muted, #8b8b90); }
  .live { font: 10px var(--mono, monospace); padding: 1px 7px; border-radius: 999px; background: var(--accent, #f2f2f2); color: var(--on-accent, #0a0a0b); }
</style>
