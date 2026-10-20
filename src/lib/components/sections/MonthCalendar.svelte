<!-- Calendario del mes: se puede avanzar o retroceder de mes y volver a hoy. Solo consulta fechas, no guarda nada. -->
<script lang="ts">
  import { WEEKDAY_LABELS, monthGrid, monthTitle } from "../../stores/week";

  let { now }: { now: Date } = $props();

  // Meses de diferencia con el actual (0 = el mes de hoy).
  let offset = $state(0);
  const first = $derived(new Date(now.getFullYear(), now.getMonth() + offset, 1));
  const weeks = $derived(monthGrid(first.getFullYear(), first.getMonth(), now));
</script>

<div class="calendar">
  <div class="bar">
    <button class="nav" aria-label="Mes anterior" onclick={() => (offset -= 1)}>‹</button>
    <button class="title" aria-label="Volver al mes de hoy" disabled={offset === 0} onclick={() => (offset = 0)}>{monthTitle(first.getFullYear(), first.getMonth())}</button>
    <button class="nav" aria-label="Mes siguiente" onclick={() => (offset += 1)}>›</button>
  </div>
  <div class="grid" role="grid" aria-label={monthTitle(first.getFullYear(), first.getMonth())}>
    <div class="row head" role="row">
      {#each WEEKDAY_LABELS as label (label)}<small role="columnheader">{label}</small>{/each}
    </div>
    {#each weeks as week, w (w)}
      <div class="row" role="row">
        {#each week as cell, i (i)}
          <span class="cell" class:out={!cell.inMonth} class:today={cell.today} role="gridcell" aria-current={cell.today ? "date" : undefined}>{cell.day}</span>
        {/each}
      </div>
    {/each}
  </div>
</div>

<style>
  .calendar { display: flex; flex-direction: column; gap: 6px; }
  .bar { display: flex; align-items: center; justify-content: space-between; gap: 6px; }
  .nav, .title { font: 12px var(--mono); color: var(--fg); background: transparent; border: 1px solid transparent; border-radius: 999px; cursor: pointer; }
  .nav { width: 26px; height: 26px; font-size: 16px; line-height: 1; padding: 0; }
  .title { padding: 4px 10px; letter-spacing: 0.06em; }
  .nav:hover, .title:not(:disabled):hover { border-color: var(--line); }
  .title:disabled { cursor: default; }
  .grid { display: flex; flex-direction: column; gap: 2px; text-align: center; }
  .row { display: grid; grid-template-columns: repeat(7, 1fr); gap: 2px; }
  .head small { opacity: 0.6; font: 10px var(--mono); text-transform: uppercase; padding-bottom: 2px; }
  .cell { padding: 5px 0; border-radius: 10px; font: 500 13px var(--mono); font-variant-numeric: tabular-nums; }
  .cell.out { opacity: 0.3; }
  .cell.today { background: var(--accent); color: var(--on-accent); box-shadow: 0 0 14px -6px var(--accent); opacity: 1; }
</style>
