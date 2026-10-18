<script lang="ts">
  import type { WindowInfo } from "../api/windows";
  import type { ExclusionRule } from "../api/exclusions";
  import type { AppSuggestion } from "../api/apps";
  import { isFullyExcluded } from "../stores/exclusions";
  import { clampPage, pageCount, slicePage } from "../stores/engine";
  import AppPicker from "./AppPicker.svelte";
  import Pager from "./Pager.svelte";

  let {
    windows,
    rules,
    onAdd,
    onRemove,
    loadApps,
  }: {
    windows: WindowInfo[];
    rules: ExclusionRule[];
    onAdd: (exe: string, title: string | null) => void;
    onRemove: (rule: ExclusionRule) => void;
    /** De dónde salen las aplicaciones del buscador (por defecto, las instaladas y abiertas). */
    loadApps?: () => Promise<AppSuggestion[]>;
  } = $props();

  /** Cuántas ventanas y cuántas exclusiones se ven por página. */
  const PER_PAGE = 10;

  let exeName = $state("");
  let titleCriterion = $state("");
  let windowsPage = $state(0);
  let rulesPage = $state(0);

  // Si la lista se acorta (se quita la última de una página) se vuelve a una página que exista.
  const winPage = $derived(clampPage(windowsPage, windows.length, PER_PAGE));
  const rulePage = $derived(clampPage(rulesPage, rules.length, PER_PAGE));

  function addTyped() {
    const exe = exeName.trim();
    if (!exe) return;
    onAdd(exe, titleCriterion.trim() || null);
    exeName = "";
    titleCriterion = "";
  }
</script>

<section>
  <h2>Ventanas abiertas</h2>
  <p class="hint">Un clic y la app deja de gestionar esa aplicación.</p>
  <table>
    <tbody>
      {#each slicePage(windows, winPage, PER_PAGE) as w (w.handle)}
        <tr>
          <td>{w.exe}</td>
          <td>{w.title}</td>
          <td>
            {#if isFullyExcluded(rules, w.exe)}
              <span class="done">Excluida</span>
            {:else}
              <button onclick={() => onAdd(w.exe, null)} disabled={!w.exe}>No gestionar</button>
            {/if}
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
  <Pager page={winPage} pages={pageCount(windows.length, PER_PAGE)} onPage={(p) => (windowsPage = p)} label="Páginas de ventanas abiertas" />
</section>

<section>
  <h2>Por aplicación</h2>
  <p class="hint">Busca entre tus aplicaciones (instaladas o abiertas) y elígela, o escribe el nombre de su proceso.</p>
  <form
    onsubmit={(e) => {
      e.preventDefault();
      addTyped();
    }}
  >
    <AppPicker bind:value={exeName} label="Nombre del proceso" placeholder="Buscar aplicación (p. ej. overwatch)" load={loadApps} onPick={(app) => (exeName = app.exe)} />
    <input placeholder="Solo si el título contiene… (opcional)" bind:value={titleCriterion} aria-label="Criterio de título" />
    <button type="submit" disabled={!exeName.trim()}>Excluir</button>
  </form>
</section>

<section>
  <h2>Excluidas ({rules.length})</h2>
  {#if rules.length === 0}
    <p class="hint">Ninguna.</p>
  {/if}
  <ul>
    {#each slicePage(rules, rulePage, PER_PAGE) as r (r.exe + "|" + (r.title_contains ?? ""))}
      <li>
        <strong>{r.exe}</strong>
        {#if r.title_contains}<span class="hint"> solo si el título contiene «{r.title_contains}»</span>{/if}
        <button onclick={() => onRemove(r)} aria-label={"Dejar de excluir " + r.exe}>Quitar</button>
      </li>
    {/each}
  </ul>
  <Pager page={rulePage} pages={pageCount(rules.length, PER_PAGE)} onPage={(p) => (rulesPage = p)} label="Páginas de aplicaciones excluidas" />
</section>

<style>
  section { margin-bottom: 1.2rem; }
  table { border-collapse: collapse; width: 100%; }
  td { padding: 4px 8px; border-bottom: 1px solid #8884; }
  form { display: flex; gap: 0.5rem; flex-wrap: wrap; align-items: center; }
  form input { padding: 4px 6px; }
  .hint { opacity: 0.7; }
  .done { opacity: 0.6; }
  ul { list-style: none; padding: 0; }
  li { padding: 4px 0; }
</style>
