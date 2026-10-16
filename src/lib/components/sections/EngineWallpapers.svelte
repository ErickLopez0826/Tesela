<!-- Fondos de Wallpaper Engine: cuadrícula con buscador, filtros, favoritos y selector de monitor. Aplicar un fondo se
     lo pide a Wallpaper Engine, que lo cambia con su propia transición. -->
<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import type { ActionOutcome, EngineLibrary, EngineWallpaper } from "../../api/system";
  import { clampPage, favoriteWallpapers, filterWallpapers, KIND_FILTERS, pageCount, slicePage, toggleFavorite, type KindFilter } from "../../stores/engine";

  let {
    library,
    onApply,
  }: {
    library: EngineLibrary;
    onApply: (id: string, monitor: number | null) => Promise<ActionOutcome>;
  } = $props();

  let query = $state("");
  let kind: KindFilter = $state("all");
  let onlyFavorites = $state(false);
  let monitor: number | null = $state(null); // null = todos
  let page = $state(0);
  let pending: string | null = $state(null);
  let outcome: ActionOutcome | null = $state(null);
  let error: string | null = $state(null);

  const monitors = $derived([...new Set(library.current.map((c) => c.monitor))].sort((a, b) => a - b));
  const filtered = $derived(filterWallpapers(library.items, { query, kind, onlyFavorites, favorites: $favoriteWallpapers }));
  const pages = $derived(pageCount(filtered.length));
  const current = $derived(clampPage(page, filtered.length));
  const visible = $derived(slicePage(filtered, current));
  // En qué monitores está puesto cada fondo (números de 1 en adelante).
  const placed = $derived(
    library.current.reduce<Record<string, number[]>>((acc, c) => ((acc[c.id] ??= []).push(c.monitor + 1), acc), {}),
  );

  // Cada cambio de búsqueda, filtro o monitor vuelve a la primera página.
  $effect(() => {
    query;
    kind;
    onlyFavorites;
    monitor;
    page = 0;
  });

  // Al cambiar de página las páginas se desplazan: si vas a la derecha, la actual sale por la izquierda y la nueva entra
  // por la derecha (y al revés). La página que sale se queda un momento como una copia sin botones.
  const SLIDE_MS = 340;
  let leaving: { items: EngineWallpaper[]; dir: 1 | -1 } | null = $state(null);
  let entering: 1 | -1 | null = $state(null);
  let slideTimer: ReturnType<typeof setTimeout> | undefined;

  function go(to: number) {
    const next = clampPage(to, filtered.length);
    if (next === current) return;
    const dir: 1 | -1 = next > current ? 1 : -1;
    leaving = { items: visible, dir };
    entering = dir;
    page = next;
    clearTimeout(slideTimer);
    slideTimer = setTimeout(() => {
      leaving = null;
      entering = null;
    }, SLIDE_MS);
  }
  $effect(() => () => clearTimeout(slideTimer));

  function src(w: EngineWallpaper): string {
    if (!w.preview) return "";
    try {
      return convertFileSrc(w.preview);
    } catch {
      return ""; // fuera de Tauri (pruebas) no hay protocolo de archivos
    }
  }

  async function apply(w: EngineWallpaper) {
    if (pending) return;
    pending = w.id;
    error = null;
    try {
      outcome = await onApply(w.id, monitor);
    } catch (e) {
      outcome = null;
      error = String(e);
    } finally {
      pending = null;
    }
  }
</script>

<section class="section engine">
  <h3>Fondos de Wallpaper Engine <span class="count">{filtered.length} de {library.items.length}</span></h3>

  <div class="controls">
    <input class="search" type="search" bind:value={query} placeholder="Buscar fondo…" aria-label="Buscar fondo" />
    <div class="chips" role="radiogroup" aria-label="Tipo de fondo">
      {#each KIND_FILTERS as k (k.id)}
        <button class="chip" role="radio" aria-checked={kind === k.id} class:active={kind === k.id} onclick={() => (kind = k.id)}>{k.label}</button>
      {/each}
      <button class="chip fav" aria-pressed={onlyFavorites} class:active={onlyFavorites} onclick={() => (onlyFavorites = !onlyFavorites)}>★ Guardados</button>
    </div>
    {#if monitors.length > 1}
      <div class="chips" role="radiogroup" aria-label="Monitor">
        <button class="chip" role="radio" aria-checked={monitor === null} class:active={monitor === null} onclick={() => (monitor = null)}>Todos los monitores</button>
        {#each monitors as m (m)}
          <button class="chip" role="radio" aria-checked={monitor === m} class:active={monitor === m} onclick={() => (monitor = m)}>Monitor {m + 1}</button>
        {/each}
      </div>
    {/if}
  </div>

  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="stage">
  {#if leaving}
    <div class="grid leaving" class:to-left={leaving.dir === 1} class:to-right={leaving.dir === -1} aria-hidden="true">
      {#each leaving.items as w (w.id)}
        <div class="card ghost">
          {#if w.preview}<img src={src(w)} alt="" decoding="async" />{:else}<span class="noimg">sin imagen</span>{/if}
          <span class="name">{w.title}</span>
        </div>
      {/each}
    </div>
  {/if}
  <div class="grid" class:from-right={entering === 1} class:from-left={entering === -1} role="group" aria-label="Fondos" onkeydown={(e) => (e.key === "ArrowRight" ? go(current + 1) : e.key === "ArrowLeft" ? go(current - 1) : null)}>
    {#each visible as w (w.id)}
      <div class="card" class:current={!!placed[w.id]} class:pending={pending === w.id}>
        <button class="apply" title={`Poner «${w.title}»`} aria-label={`Poner ${w.title}`} disabled={pending !== null} onclick={() => apply(w)}>
          {#if w.preview}<img src={src(w)} alt="" loading="lazy" decoding="async" />{:else}<span class="noimg">sin imagen</span>{/if}
          <span class="name">{pending === w.id ? "Aplicando…" : w.title}</span>
        </button>
        {#if placed[w.id]}<span class="now" title="Puesto ahora">{placed[w.id].join(" · ")}</span>{/if}
        {#if w.kind}<span class="kind">{w.kind}</span>{/if}
        <button class="star" class:on={$favoriteWallpapers.has(w.id)} aria-label={$favoriteWallpapers.has(w.id) ? `Quitar ${w.title} de guardados` : `Guardar ${w.title}`} aria-pressed={$favoriteWallpapers.has(w.id)} onclick={() => toggleFavorite(w.id)}>★</button>
      </div>
    {:else}
      <div class="hint empty">{onlyFavorites && $favoriteWallpapers.size === 0 ? "Aún no has guardado ningún fondo: marca la estrella de los que quieras." : "Ningún fondo coincide."}</div>
    {/each}
  </div>
  </div>
  {#if pages > 1}
    <nav class="pager" aria-label="Páginas de fondos">
      <button class="btn" onclick={() => go(current - 1)} disabled={current === 0}>‹ Anterior</button>
      <span class="where">Página {current + 1} de {pages}</span>
      <button class="btn" onclick={() => go(current + 1)} disabled={current >= pages - 1}>Siguiente ›</button>
    </nav>
  {/if}

  {#if outcome}<div class="notice">{outcome.description}</div>{/if}
  {#if error}<div class="notice error">{error}</div>{/if}
</section>

<style>
  .engine { display: flex; flex-direction: column; gap: 8px; grid-column: 1 / -1; }
  h3 { display: flex; }
  .count { margin-left: auto; letter-spacing: 0.06em; text-transform: none; font-weight: 400; }
  .controls { display: flex; flex-direction: column; gap: 6px; }
  .search { font: 13px "Segoe UI", system-ui, sans-serif; color: var(--fg); background: rgba(0, 0, 0, 0.5); border: 1px solid var(--line); border-radius: 12px; padding: 7px 10px; box-sizing: border-box; width: 100%; }
  .search:focus { outline: none; border-color: var(--accent); }
  .chips { display: flex; flex-wrap: wrap; gap: 4px; }
  .chip { padding: 3px 10px; font: 10px var(--mono); letter-spacing: 0.06em; text-transform: uppercase; color: var(--fg); background: var(--panel); border: 1px solid var(--line); border-radius: 999px; cursor: pointer; }
  .chip.active { background: var(--accent); border-color: var(--accent); color: var(--on-accent); }
  .stage { position: relative; overflow: hidden; border-radius: 14px; }
  .grid { display: grid; grid-template-columns: repeat(6, minmax(0, 1fr)); gap: 8px; padding: 2px; }
  .grid.leaving { position: absolute; inset: 0 0 auto 0; pointer-events: none; animation: page-out 0.34s cubic-bezier(.22, 1, .36, 1) both; }
  .grid.leaving.to-right { animation-name: page-out-right; }
  .grid.from-right { animation: page-in-right 0.34s cubic-bezier(.22, 1, .36, 1) both; }
  .grid.from-left { animation: page-in-left 0.34s cubic-bezier(.22, 1, .36, 1) both; }
  @keyframes page-out { to { transform: translateX(-100%); } }
  @keyframes page-out-right { to { transform: translateX(100%); } }
  @keyframes page-in-right { from { transform: translateX(100%); } }
  @keyframes page-in-left { from { transform: translateX(-100%); } }
  .ghost { content-visibility: visible; }
  @media (prefers-reduced-motion: reduce) { .grid.leaving, .grid.from-right, .grid.from-left { animation: none; } }
  .empty { grid-column: 1 / -1; padding: 8px 2px; }
  .card { position: relative; content-visibility: auto; contain-intrinsic-size: 132px 104px; border-radius: 14px; overflow: hidden; border: 1px solid var(--line); background: #111; }
  .card.current { border-color: var(--accent); box-shadow: 0 0 16px -6px var(--accent); }
  .card.pending { opacity: 0.6; }
  .apply { display: block; width: 100%; padding: 0; border: 0; border-radius: 0; background: transparent; text-align: left; text-transform: none; letter-spacing: 0; cursor: pointer; position: relative; }
  .apply img, .noimg { display: grid; place-items: center; width: 100%; aspect-ratio: 16 / 10; object-fit: cover; background: #1a1a1c; color: var(--muted); font: 10px var(--mono); }
  .name { position: absolute; inset: auto 0 0 0; padding: 14px 8px 5px; font: 11px "Segoe UI", system-ui, sans-serif; color: #fff; background: linear-gradient(transparent, rgba(0, 0, 0, 0.85)); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .kind { position: absolute; top: 5px; left: 5px; padding: 1px 7px; border-radius: 999px; background: rgba(0, 0, 0, 0.6); color: #fff; font: 9px var(--mono); text-transform: uppercase; }
  .now { position: absolute; top: 5px; right: 34px; padding: 1px 7px; border-radius: 999px; background: var(--accent); color: var(--on-accent); font: 700 10px var(--mono); }
  .star { position: absolute; top: 3px; right: 4px; width: 24px; height: 24px; padding: 0; display: grid; place-items: center; border-radius: 50%; border: 0; background: rgba(0, 0, 0, 0.55); color: rgba(255, 255, 255, 0.5); font-size: 13px; cursor: pointer; }
  .star.on { color: var(--accent); }
  .pager { display: flex; align-items: center; justify-content: center; gap: 12px; }
  .where { font: 11px var(--mono); letter-spacing: 0.08em; color: var(--muted); }
</style>
