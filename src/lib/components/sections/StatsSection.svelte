<script lang="ts">
  import type { SystemOverview } from "../../api/system";
  import { formatBytes, formatRate, formatUptime, percentOf } from "../../stores/format";
  import { formatPercent } from "../../stores/wmState";

  let { overview }: { overview: SystemOverview | null } = $props();
</script>

<section class="section">
  <h3>Estadísticas</h3>
  {#if !overview}
    <div class="hint">Leyendo…</div>
  {:else}
    <div class="row"><span>CPU</span><span class="value">{formatPercent(overview.cpu_percent)}</span></div>
    <div class="meter" role="progressbar" aria-label="CPU" aria-valuenow={Math.round(overview.cpu_percent)}>
      <div class="fill" style:width={`${overview.cpu_percent}%`}></div>
    </div>

    <div class="row">
      <span>GPU</span>
      <span class="value">{overview.gpu_percent === null ? "no disponible" : formatPercent(overview.gpu_percent)}</span>
    </div>
    {#if overview.gpu_percent !== null}
      <div class="meter" role="progressbar" aria-label="GPU" aria-valuenow={Math.round(overview.gpu_percent)}>
        <div class="fill" style:width={`${overview.gpu_percent}%`}></div>
      </div>
    {/if}

    <div class="row">
      <span>RAM</span>
      <span class="value">{formatBytes(overview.ram_used)} de {formatBytes(overview.ram_total)}</span>
    </div>
    <div class="meter" role="progressbar" aria-label="RAM" aria-valuenow={Math.round(percentOf(overview.ram_used, overview.ram_total))}>
      <div class="fill" style:width={`${percentOf(overview.ram_used, overview.ram_total)}%`}></div>
    </div>

    {#each overview.disks as d (d.name)}
      <div class="row"><span>Disco {d.name}</span><span class="value">{formatBytes(d.used)} de {formatBytes(d.total)}</span></div>
      <div class="meter" class:warn={percentOf(d.used, d.total) > 90} role="progressbar" aria-label={`Disco ${d.name}`} aria-valuenow={Math.round(percentOf(d.used, d.total))}>
        <div class="fill" style:width={`${percentOf(d.used, d.total)}%`}></div>
      </div>
    {/each}

    <div class="row"><span>Red</span><span class="value">↓ {formatRate(overview.net_down)} · ↑ {formatRate(overview.net_up)}</span></div>
    <div class="row"><span>Encendido</span><span class="value">{formatUptime(overview.uptime_secs)}</span></div>
    {#if overview.temperature_c !== null}
      <div class="row"><span>Temperatura</span><span class="value">{Math.round(overview.temperature_c)} °C</span></div>
    {/if}

    <h3 class="top">Apps que más consumen</h3>
    {#each overview.top_processes as p (p.name)}
      <div class="row proc">
        <span class="name">{p.name}</span>
        <span class="value">{formatPercent(p.cpu_percent)} · {formatBytes(p.ram_bytes)}</span>
      </div>
    {/each}
  {/if}
</section>

<style>
  .top { margin-top: 12px !important; }
  .proc .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
