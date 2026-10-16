<!-- Anillo de porcentaje (CPU, GPU, RAM). Un valor nulo se muestra vacío con «—». Debajo, el nombre del componente y
     unas fichas con sus especificaciones. -->
<script lang="ts">
  let {
    value,
    label,
    detail = "",
    chips = [],
    warn = 85,
  }: { value: number | null; label: string; detail?: string; chips?: string[]; warn?: number } = $props();

  const R = 38;
  const C = 2 * Math.PI * R;
  const pct = $derived(value === null ? 0 : Math.max(0, Math.min(100, value)));
</script>

<div class="gauge" role="progressbar" aria-label={label} aria-valuenow={value === null ? undefined : Math.round(pct)}>
  <div class="ring">
    <svg viewBox="0 0 100 100" width="92" height="92">
      <circle class="track" cx="50" cy="50" r={R} />
      <circle class="arc" class:hot={pct >= warn} cx="50" cy="50" r={R} stroke-dasharray={C} stroke-dashoffset={C * (1 - pct / 100)} />
    </svg>
    <div class="center">{value === null ? "—" : `${Math.round(pct)}%`}</div>
  </div>
  <div class="label">{label}</div>
  {#if detail}<div class="detail" title={detail}>{detail}</div>{/if}
  {#if chips.length}
    <div class="chips">
      {#each chips as c (c)}<span class="chip">{c}</span>{/each}
    </div>
  {/if}
</div>

<style>
  .gauge { display: flex; flex-direction: column; align-items: center; gap: 3px; min-width: 0; }
  .ring { position: relative; width: 92px; height: 92px; }
  svg { transform: rotate(-90deg); }
  .track { fill: none; stroke: rgba(255, 255, 255, 0.08); stroke-width: 7; stroke-dasharray: 1 6; stroke-linecap: round; }
  .arc { fill: none; stroke: var(--accent, #f2f2f2); stroke-width: 7; stroke-linecap: round; transition: stroke-dashoffset 0.9s cubic-bezier(.3, 1.1, .4, 1), stroke 0.4s; filter: drop-shadow(0 0 4px rgba(255, 255, 255, 0.45)); }
  .arc.hot { animation: hot 1.4s ease-in-out infinite; }
  @keyframes hot { 50% { opacity: 0.45; } }
  .center { position: absolute; inset: 0; display: grid; place-items: center; font: 500 17px var(--mono); }
  .label { font: 600 12px var(--mono); letter-spacing: 0.14em; }
  .detail { max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--muted); font: 10px var(--mono); text-align: center; }
  .chips { display: flex; flex-wrap: wrap; justify-content: center; gap: 4px; margin-top: 2px; }
  .chip { padding: 2px 8px; border-radius: 999px; background: rgba(255, 255, 255, 0.08); border: 1px solid var(--line); font: 10px var(--mono); }
</style>
