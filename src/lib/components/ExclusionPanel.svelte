<script lang="ts">
  import type { WindowInfo } from "../api/windows";
  import type { ExclusionRule } from "../api/exclusions";
  import { isFullyExcluded } from "../stores/exclusions";

  let {
    windows,
    rules,
    onAdd,
    onRemove,
  }: {
    windows: WindowInfo[];
    rules: ExclusionRule[];
    onAdd: (exe: string, title: string | null) => void;
    onRemove: (rule: ExclusionRule) => void;
  } = $props();

  let exeName = $state("");
  let titleCriterion = $state("");

  function addTyped() {
    const exe = exeName.trim();
    if (!exe) return;
    onAdd(exe, titleCriterion.trim() || null);
    exeName = "";
    titleCriterion = "";
  }

  // El navegador solo entrega el nombre del archivo, y con eso basta: la exclusión es por nombre de ejecutable.
  function pickFile(event: Event) {
    const file = (event.currentTarget as HTMLInputElement).files?.[0];
    if (file) exeName = file.name;
  }
</script>

<section>
  <h2>Ventanas abiertas</h2>
  <p class="hint">Un clic y la app deja de gestionar esa aplicación.</p>
  <table>
    <tbody>
      {#each windows as w (w.handle)}
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
</section>

<section>
  <h2>Por ejecutable</h2>
  <form
    onsubmit={(e) => {
      e.preventDefault();
      addTyped();
    }}
  >
    <input placeholder="Nombre del proceso (p. ej. overwatch)" bind:value={exeName} aria-label="Nombre del proceso" />
    <label class="file">
      Elegir .exe…
      <input type="file" accept=".exe" onchange={pickFile} hidden />
    </label>
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
    {#each rules as r (r.exe + "|" + (r.title_contains ?? ""))}
      <li>
        <strong>{r.exe}</strong>
        {#if r.title_contains}<span class="hint"> solo si el título contiene «{r.title_contains}»</span>{/if}
        <button onclick={() => onRemove(r)} aria-label={"Dejar de excluir " + r.exe}>Quitar</button>
      </li>
    {/each}
  </ul>
</section>

<style>
  section { margin-bottom: 1.2rem; }
  table { border-collapse: collapse; width: 100%; }
  td { padding: 4px 8px; border-bottom: 1px solid #8884; }
  form { display: flex; gap: 0.5rem; flex-wrap: wrap; align-items: center; }
  input:not([hidden]) { padding: 4px 6px; }
  .file { cursor: pointer; text-decoration: underline; }
  .hint { opacity: 0.7; }
  .done { opacity: 0.6; }
  ul { list-style: none; padding: 0; }
  li { padding: 4px 0; }
</style>
