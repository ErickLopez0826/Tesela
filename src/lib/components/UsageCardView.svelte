<!-- Tarjeta que sale al pasar el cursor por el uso de Claude en la barra: cada ventana con su reinicio, una barra de puntos y el porcentaje,
     lo que Claude tiene en marcha ahora (sesiones y subagentes, con su origen) y el desglose semanal por producto. Es informativa: no toma
     el foco y el núcleo la cierra cuando el cursor se va de ella y del anillo. Sigue la personalización del panel: cristal con la transparencia
     y el desenfoque elegidos, color de los puntos (que se encienden cerca del cursor) y texto claro u oscuro según lo que haya detrás. -->
<script lang="ts">
  import { onMount } from "svelte";
  import type { ClaudeActivity, UsageLevel, UsageView } from "../api/claude";
  import { formatAge, isOutdated, resetLabel, windowTitle } from "../stores/claude";
  import { theme } from "../stores/theme";
  import { effectiveLuma, toneFor } from "../stores/tone";
  import ClaudeMark from "./ClaudeMark.svelte";
  import DotField from "./DotField.svelte";
  import GlassBackdrop from "./GlassBackdrop.svelte";

  let {
    load,
    loadActivity = () => Promise.resolve(null),
    resize = () => {},
    openTick = 0,
    closeTick = 0,
  }: {
    /** Lee el uso ya calculado por el núcleo (no hace ninguna petición a la red). */
    load: () => Promise<UsageView>;
    /** Lee lo que Claude tiene en marcha (los avisos de Claude Code); se repite mientras la tarjeta está abierta. */
    loadActivity?: () => Promise<ClaudeActivity | null>;
    /** Avisa del tamaño (lógico) que necesita el contenido. */
    resize?: (width: number, height: number) => void | Promise<void>;
    /** Sube cada vez que el núcleo la abre. */
    openTick?: number;
    /** Sube cuando el núcleo empieza a cerrarla. */
    closeTick?: number;
  } = $props();

  const BAR_DOTS = 28;
  const ACTIVITY_REFRESH_MS = 2000;

  let usage: UsageView | null = $state(null);
  let activity: ClaudeActivity | null = $state(null);
  let phase: "open" | "closing" = $state("open");
  let box: HTMLElement | undefined = $state();
  let now = $state(Math.floor(Date.now() / 1000));
  let seenOpen = 0;
  let seenClose = 0;

  // El texto se adapta al fondo: claro sobre oscuro y oscuro sobre claro (tinte de la tarjeta + lo que se ve a través).
  let behind: number | null = $state(null);
  const tone = $derived(toneFor(effectiveLuma(behind, 1 - $theme.panelTransparency / 100)));

  const refreshActivity = () =>
    loadActivity()
      .then((a) => (activity = a))
      .catch(() => {});

  $effect(() => {
    if (openTick !== seenOpen) {
      seenOpen = openTick;
      phase = "open";
      now = Math.floor(Date.now() / 1000);
      load().then((u) => (usage = u)).catch(() => {});
      void refreshActivity();
    }
    if (closeTick !== seenClose) {
      seenClose = closeTick;
      phase = "closing";
    }
  });

  // Mientras está abierta se vuelve a leer lo que hay en marcha (y la hora de los reinicios).
  $effect(() => {
    if (phase !== "open" || openTick === 0) return;
    const timer = setInterval(() => {
      now = Math.floor(Date.now() / 1000);
      void refreshActivity();
    }, ACTIVITY_REFRESH_MS);
    return () => clearInterval(timer);
  });

  onMount(() => {
    if (!box) return;
    let last = "";
    const measure = () => {
      if (!box) return;
      const w = Math.ceil(box.offsetWidth);
      const h = Math.ceil(box.offsetHeight);
      if (`${w}x${h}` === last) return;
      last = `${w}x${h}`;
      void Promise.resolve(resize(w, h)).catch(() => {});
    };
    measure();
    const observer = typeof ResizeObserver === "function" ? new ResizeObserver(measure) : null;
    observer?.observe(box);
    return () => observer?.disconnect();
  });

  /** Puntos encendidos de una barra: al menos uno si se ha gastado algo. */
  const lit = (percent: number) => (percent <= 0 ? 0 : Math.min(BAR_DOTS, Math.max(1, Math.round((percent / 100) * BAR_DOTS))));
  const levelOf = (l: UsageLevel) => l;
</script>

<div class="stage" class:closing={phase === "closing"} class:alt={openTick % 2 === 1}>
  <div class="box" bind:this={box} aria-label="Uso de Claude" style:--fg={tone.fg} style:--muted={tone.muted}>
    <GlassBackdrop kind="popup" blur={$theme.panelBlur} tick={openTick} onLuma={(l) => (behind = l)} />
    <div class="tint" aria-hidden="true"></div>
    <DotField fixed={false} />
    <div class="content">
    <header>
      <span class="mark"><ClaudeMark size={18} /></span>
      <b>Uso de Claude</b>
    </header>

    <div class="cols">
    <div class="col">
    {#if !usage || usage.windows.length === 0}
      <p class="empty">{usage?.message ?? "Sin datos todavía."}</p>
    {:else}
      {#each usage.windows as w (w.id)}
        {@const old = isOutdated(w, now)}
        <section class="win {levelOf(w.level)}" class:old>
          <div class="line">
            <span class="name">{windowTitle(w)}</span>
            <span class="reset">{resetLabel(w, now)}</span>
          </div>
          <div class="bar" role="img" aria-label={old ? "reiniciada" : `${Math.round(w.percent)}% usado`}>
            {#each Array.from({ length: BAR_DOTS }) as _, i (i)}<i class:on={!old && i < lit(w.percent)}></i>{/each}
          </div>
          <span class="used">{old ? "—" : `${Math.round(w.percent)}% usado`}</span>
        </section>
      {/each}
    {/if}
    </div>

    <div class="col">
    {#if activity}
      <div class="live" aria-label="Lo que Claude tiene en marcha">
        <p class="cap">En marcha</p>
        {#each activity.agents ?? [] as a, i (i)}
          <div class="agent" class:sub={a.sub} class:waiting={a.state === "waiting"}>
            <i class="pulse" aria-hidden="true"></i>
            <span class="aname">{a.name}</span>
            <span class="asrc">{a.state === "waiting" ? "te espera · " : ""}{a.source}</span>
          </div>
        {:else}
          <p class="none">Nada trabajando ahora.</p>
        {/each}
      </div>
    {/if}

    {#if usage && usage.windows.length > 0 && usage.breakdown.length > 0}
      <p class="share">Esta semana · {usage.breakdown.map((s) => `${s.name} ${Math.round(s.percent)}%`).join(" · ")}</p>
    {/if}
    </div>
    </div>

    {#if usage && usage.windows.length > 0}
      {#if usage.fetched_at !== null}
        <p class="foot">Leído {formatAge(usage.fetched_at, now)}{usage.status === "stale" && usage.message ? ` · ${usage.message}` : ""}</p>
      {/if}
    {/if}
    </div>
  </div>
</div>

<style>
  :global(html, body) { margin: 0; height: 100%; background: transparent; overflow: hidden; }
  .stage { position: fixed; inset: 0; display: flex; align-items: flex-start; justify-content: flex-end; animation: pop-in 0.4s cubic-bezier(.22, 1, .36, 1) both; }
  .stage.alt { animation-name: pop-in-b; }
  .stage.closing, .stage.alt.closing { animation: pop-out 0.3s cubic-bezier(.22, 1, .36, 1) both; pointer-events: none; }
  @keyframes pop-in { from { opacity: 0; transform: translateY(-8px) scale(0.96); } to { opacity: 1; transform: none; } }
  @keyframes pop-in-b { from { opacity: 0; transform: translateY(-8px) scale(0.96); } to { opacity: 1; transform: none; } }
  @keyframes pop-out { from { opacity: 1; transform: none; } to { opacity: 0; transform: translateY(-8px) scale(0.96); } }
  /* Cristal como el del panel: la copia de lo que hay detrás, el tinte con la opacidad de Personalización y los puntos (DotField). */
  .box {
    position: relative; isolation: isolate; overflow: hidden;
    box-sizing: border-box; flex: none; width: 100%; color: var(--fg); font: 11px var(--mono);
    border: 1px solid var(--line, rgba(255, 255, 255, 0.14)); border-radius: 20px;
  }
  .tint { position: absolute; inset: 0; z-index: 0; pointer-events: none; background: rgba(10, 10, 11, var(--panel-alpha, 1)); }
  .content { position: relative; z-index: 1; padding: 14px 20px 12px; }
  /* Ancha: las ventanas de uso a un lado y lo que está en marcha al otro. En una ventana estrecha, una debajo de otra. */
  .cols { display: grid; grid-template-columns: 1fr 1fr; gap: 0 32px; align-items: start; }
  .col { min-width: 0; }
  header { display: flex; align-items: center; gap: 8px; margin-bottom: 12px; }
  header b { font: 600 12px var(--mono); letter-spacing: 0.1em; text-transform: uppercase; }
  .mark { color: #d97757; display: grid; }
  .win { display: block; margin-bottom: 12px; padding: 0; }
  .line { display: flex; justify-content: space-between; align-items: baseline; gap: 10px; margin-bottom: 6px; }
  .name { font-weight: 600; letter-spacing: 0.03em; white-space: nowrap; }
  .reset { color: var(--muted); font-size: 10px; white-space: nowrap; }
  .bar { display: grid; grid-template-columns: repeat(28, 1fr); gap: 2px; align-items: center; }
  .bar i { aspect-ratio: 1; border-radius: 50%; background: currentColor; opacity: 0.18; }
  .bar i.on { opacity: 1; }
  .warn .bar i.on { background: #ffb347; }
  .danger .bar i.on { background: #ff6b5e; }
  .used { display: block; margin-top: 6px; font: 600 12px var(--mono); font-variant-numeric: tabular-nums; }
  .warn .used { color: #ffb347; }
  .danger .used { color: #ff6b5e; }
  .old { opacity: 0.55; }
  .live { margin: 0 0 8px; }
  @media (max-width: 560px) {
    .cols { grid-template-columns: 1fr; }
    .live { padding-top: 8px; border-top: 1px dotted var(--line, rgba(255, 255, 255, 0.14)); }
  }
  .cap { margin: 0 0 6px; color: var(--muted); font-size: 10px; letter-spacing: 0.1em; text-transform: uppercase; }
  .agent { display: flex; align-items: center; gap: 8px; padding: 2px 0; }
  .agent.sub { padding-left: 16px; }
  .pulse { flex: none; width: 6px; height: 6px; border-radius: 50%; background: #d97757; animation: agent-pulse 1.2s ease-in-out infinite; }
  .waiting .pulse { background: #ffb347; }
  .aname { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 600; }
  .asrc { flex: none; color: var(--muted); font-size: 10px; white-space: nowrap; }
  .none { margin: 0; color: var(--muted); }
  @keyframes agent-pulse { 50% { transform: scale(0.55); opacity: 0.55; } }
  .share { margin: 2px 0 8px; padding-top: 8px; border-top: 1px dotted var(--line, rgba(255, 255, 255, 0.14)); color: var(--muted); font-size: 10px; line-height: 1.5; }
  .foot { margin: 0; color: var(--muted); font-size: 10px; }
  .empty { margin: 0 0 8px; color: var(--muted); }
  @media (prefers-reduced-motion: reduce) { .stage, .stage.closing { animation: none; } .pulse { animation: none; } }
</style>
