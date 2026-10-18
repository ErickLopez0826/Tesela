<!-- Paginación sencilla: anterior, «2 / 5» y siguiente. No se muestra si todo cabe en una página. -->
<script lang="ts">
  let { page, pages, onPage, label = "Páginas" }: { page: number; pages: number; onPage: (page: number) => void; label?: string } = $props();
</script>

{#if pages > 1}
  <nav class="pager" aria-label={label}>
    <button type="button" aria-label="Página anterior" disabled={page <= 0} onclick={() => onPage(page - 1)}>‹</button>
    <span class="count">{page + 1} / {pages}</span>
    <button type="button" aria-label="Página siguiente" disabled={page >= pages - 1} onclick={() => onPage(page + 1)}>›</button>
  </nav>
{/if}

<style>
  .pager { display: flex; align-items: center; justify-content: center; gap: 10px; margin: 8px 0 2px; }
  .count { font: 12px var(--mono, monospace); color: var(--muted, #8b8b90); min-width: 48px; text-align: center; }
  button { min-width: 34px; }
</style>
