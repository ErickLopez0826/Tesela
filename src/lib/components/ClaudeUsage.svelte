<!-- Uso de la suscripción de Claude en la barra superior: el anillo de puntos con la marca de Claude y el porcentaje de la sesión (5 horas).
     Al pasar el cursor se abre una tarjeta con todo el detalle (ver `UsageCardView`). Con una lectura vieja se ve atenuado. -->
<script lang="ts">
  import { onMount } from "svelte";
  import type { UsageView } from "../api/claude";
  import { describeUsage, isOutdated, usageLevel } from "../stores/claude";
  import UsageRing from "./UsageRing.svelte";

  let {
    usage,
    onHover = () => {},
    onLeave = () => {},
    working = false,
  }: {
    usage: UsageView;
    /** Claude está trabajando ahora mismo (según los avisos de Claude Code): el anillo se anima. */
    working?: boolean;
    /** El cursor entra: recibe el rectángulo del anillo (px lógicos de la ventana) para colocar la tarjeta. */
    onHover?: (anchor: { x: number; y: number; w: number; h: number }) => void;
    onLeave?: () => void;
  } = $props();

  // Cada medio minuto se recalcula lo que depende de la hora (reinicios vencidos) sin pedir nada al núcleo.
  let now = $state(Math.floor(Date.now() / 1000));
  onMount(() => {
    const timer = setInterval(() => (now = Math.floor(Date.now() / 1000)), 30_000);
    return () => clearInterval(timer);
  });

  const session = $derived(usage.windows.find((w) => w.id === "five_hour") ?? usage.windows[0] ?? null);
  const percent = $derived(session && !isOutdated(session, now) ? session.percent : null);
  const level = $derived(usageLevel(usage, now));
  const label = $derived(`Uso de tu suscripción de Claude. ${describeUsage(usage, now).replaceAll("\n", ". ")}`);

  let el: HTMLElement | undefined = $state();
  function enter() {
    const r = el?.getBoundingClientRect();
    if (r) onHover({ x: r.left, y: r.top, w: r.width, h: r.height });
  }
</script>

<span class="claude-usage {level}" class:stale={usage.status === "stale"} class:working role="img" aria-label={label} bind:this={el} onmouseenter={enter} onmouseleave={() => onLeave()}>
  <UsageRing {percent} {level} size={24} dim={usage.status === "stale"} {working} />
  <span class="value">{percent === null ? "—" : `${Math.round(percent)}%`}</span>
</span>

<style>
  /* Misma intensidad que el resto de datos de la barra (`.stat` de BarView). */
  .claude-usage { display: inline-flex; align-items: center; gap: 6px; padding: 0 2px; font-variant-numeric: tabular-nums; cursor: default; opacity: 0.8; }
  .warn .value { color: #ffb347; }
  .danger .value { color: #ff6b5e; }
</style>
