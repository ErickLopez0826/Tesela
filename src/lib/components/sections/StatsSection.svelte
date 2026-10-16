<script lang="ts">
  import Gauge from "../Gauge.svelte";
  import Icon from "../Icon.svelte";
  import type { SystemOverview } from "../../api/system";
  import { formatBytes, percentOf } from "../../stores/format";
  import { formatMetric, metricValue, METRICS, rankProcesses, type ProcessMetric } from "../../stores/processes";

  let { overview }: { overview: SystemOverview | null } = $props();

  /** «4.2 GHz», «8 núcleos / 16 hilos», temperatura… para las fichas del procesador. */
  const cpuChips = $derived.by(() => {
    if (!overview) return [];
    const h = overview.hardware;
    const chips: string[] = [];
    if (h.cpu_mhz > 0) chips.push(`${(h.cpu_mhz / 1000).toFixed(1)} GHz`);
    if (h.cpu_cores > 0) chips.push(`${h.cpu_cores}N / ${h.cpu_threads}H`);
    if (overview.temperature_c !== null) chips.push(`${Math.round(overview.temperature_c)} °C`);
    return chips;
  });
  const gpuChips = $derived.by(() => {
    const h = overview?.hardware;
    return h && h.gpu_vram > 0 ? [`${formatBytes(h.gpu_vram)} VRAM`] : [];
  });

  let metric: ProcessMetric = $state("ram");
  const ranked = $derived(rankProcesses(overview?.top_processes ?? [], metric));
  const maxValue = $derived(Math.max(1e-9, ...ranked.map((p) => metricValue(p, metric))));
</script>

{#if !overview}
  <section class="section"><div class="hint">Leyendo…</div></section>
{:else}
  <div class="stats">
    <div class="left">
      <section class="section rings">
        <Gauge value={overview.cpu_percent} label="CPU" detail={overview.hardware.cpu_name} chips={cpuChips} />
        <Gauge value={overview.gpu_percent} label="GPU" detail={overview.hardware.gpu_name ?? "no disponible"} chips={gpuChips} />
        <Gauge value={percentOf(overview.ram_used, overview.ram_total)} label="RAM" detail={`${formatBytes(overview.ram_used)} de ${formatBytes(overview.ram_total)}`} />
      </section>

      <section class="section disks">
        <h3>Discos</h3>
        {#each overview.disks as d (d.name)}
          <div class="disk">
            <div class="line"><span class="name">{d.name}</span><span class="value">{formatBytes(d.used)} de {formatBytes(d.total)}</span></div>
            <div class="meter" class:warn={percentOf(d.used, d.total) > 90} role="progressbar" aria-label={`Disco ${d.name}`} aria-valuenow={Math.round(percentOf(d.used, d.total))}>
              <div class="fill" style:width={`${percentOf(d.used, d.total)}%`}></div>
            </div>
          </div>
        {/each}
      </section>
    </div>

    <section class="section apps">
      <h3>Apps que más consumen</h3>
      <div class="filters" role="radiogroup" aria-label="Ordenar por">
        {#each METRICS as m (m.id)}
          <button class="filter" role="radio" aria-checked={metric === m.id} class:active={metric === m.id} onclick={() => (metric = m.id)}>{m.label}</button>
        {/each}
      </div>
      {#each ranked as p (p.name)}
        <div class="proc">
          <div class="line"><span class="name">{p.name}</span><span class="value">{formatMetric(metricValue(p, metric), metric)}</span></div>
          <div class="meter"><div class="fill" style:width={`${(metricValue(p, metric) / maxValue) * 100}%`}></div></div>
        </div>
      {:else}
        <div class="hint">Ninguna app usa {METRICS.find((m) => m.id === metric)?.label} ahora mismo.</div>
      {/each}
    </section>
  </div>
{/if}

<style>
  .stats { display: grid; grid-template-columns: 1.5fr 1fr; gap: 12px; align-items: start; }
  .rings { display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; }
  .left { display: flex; flex-direction: column; gap: 12px; min-width: 0; }
  .disk + .disk { margin-top: 10px; }
  .filters { display: flex; gap: 4px; margin-bottom: 10px; }
  .filter { padding: 3px 10px; font-size: 10px; }
  .filter.active { background: var(--accent); border-color: var(--accent); color: var(--on-accent); }
  .proc { margin-bottom: 8px; }
  .line { display: flex; justify-content: space-between; gap: 8px; margin-bottom: 3px; font-size: 12px; }
  .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .apps .meter { height: 5px; }
</style>
