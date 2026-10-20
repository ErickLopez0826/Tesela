<!-- Barra de tareas de Tesela: una isla centrada abajo con las ventanas de cada workspace (1, 2, 3…), cada una con su
     icono. Su ancho sigue a su contenido y cambia con una transición suave; la ventana se ajusta al mismo ancho. Detrás
     lleva el mismo fondo de puntos que el panel: se ilumina bajo el cursor y una ola de luz sale de cada botón que se pulsa. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { scale } from "svelte/transition";
  import type { StateSnapshot } from "../api/state";
  import type { BackgroundApp, TaskbarApi } from "../api/shell";
  import type { ActionOutcome, PowerActionName } from "../api/system";
  import { backgroundLabel, groupsFor, loadExeIcon, loadIcon, windowLabel } from "../stores/taskbar";
  import { formatBytes } from "../stores/format";
  import { theme } from "../stores/theme";
  import DotField from "./DotField.svelte";
  import GlassBackdrop from "./GlassBackdrop.svelte";
  import Icon from "./Icon.svelte";
  import PowerMenu from "./PowerMenu.svelte";

  let {
    state: desktop,
    monitorId,
    api,
    onWorkspace,
    openTick = 0,
    closeTick = 0,
    background = [],
    power = null,
  }: {
    state: StateSnapshot | null;
    monitorId: number;
    api: TaskbarApi;
    onWorkspace: (n: number) => void;
    /** Sube cada vez que el núcleo despliega la barra: se reproduce la animación de aparición. */
    openTick?: number;
    /** Sube cuando el núcleo la cierra: se reproduce la animación al revés. */
    closeTick?: number;
    /** Aplicaciones en segundo plano (bandeja); vacío = sin sección. */
    background?: BackgroundApp[];
    /** Energía: sin esto no hay botón. */
    power?: { allowChanges: boolean; run: (action: PowerActionName) => Promise<ActionOutcome> } | null;
  } = $props();

  const SHRINK_DELAY_MS = 380;
  /** Separación (lógica) entre la isla y el borde de abajo del monitor; la misma que usa el núcleo para colocarla. */
  const BOTTOM_MARGIN = 14;

  const groups = $derived(groupsFor(desktop, monitorId));

  /** `open`: aparece desde abajo; `closing`: la misma animación al revés. */
  let phase: "open" | "closing" = $state("open");
  let seenOpen = 0;
  let seenClose = 0;
  $effect(() => {
    if (openTick !== seenOpen) {
      seenOpen = openTick;
      phase = "open";
      // El cristal copia lo de detrás en la posición definitiva de la ventana.
      glassTick++;
    }
    if (closeTick !== seenClose) {
      seenClose = closeTick;
      phase = "closing";
    }
  });

  /** La lista de aplicaciones en segundo plano está desplegada (sube sobre la barra, no se ensancha). */
  let bgOpen = $state(false);
  /** El menú de energía está abierto: la barra solo muestra sus botones y uno para volver. */
  let powerOpen = $state(false);
  // Sin aplicaciones en segundo plano no hay nada que desplegar.
  $effect(() => {
    if (background.length === 0) bgOpen = false;
  });
  // Al cerrarse la barra todo vuelve a su sitio.
  $effect(() => {
    if (closeTick > 0) {
      bgOpen = false;
      powerOpen = false;
    }
  });

  /** El botón «⋯» abre la ventana emergente de esa app, junto al botón (o la cierra si ya estaba abierta para ella). */
  function openActions(a: BackgroundApp, event: MouseEvent) {
    const r = (event.currentTarget as HTMLElement).getBoundingClientRect();
    void Promise.resolve(api.openPopup(a, { x: r.left, y: r.top, w: r.width, h: r.height })).catch(() => {});
  }

  /** Tocar una app en segundo plano la abre o la enfoca y cierra la barra. */
  function openBackground(a: BackgroundApp) {
    void Promise.resolve(api.openBackground(a.exe)).catch(() => {});
    void Promise.resolve(api.hide()).catch(() => {});
  }

  /** Tocar una app la enfoca y cierra la barra. */
  function open(id: number) {
    void Promise.resolve(api.focus(id)).catch(() => {});
    void Promise.resolve(api.hide()).catch(() => {});
  }

  let content: HTMLElement | undefined = $state();
  /** Tamaño de la isla; cambia con transición. */
  let width: number | null = $state(null);
  let height: number | null = $state(null);
  let settled = $state(false);
  /** Sube al desplegarse la barra: el cristal vuelve a pedir su imagen (ya copiada: el tamaño de la isla no la invalida). */
  let glassTick = $state(0);
  let shrinkTimer: ReturnType<typeof setTimeout> | undefined;
  let lastAsked = { w: 0, h: 0 };

  /** Pide a la ventana el tamaño nuevo: si crece, ya; si encoge, al terminar la transición de la isla. */
  function resizeWindow(w: number, h: number) {
    clearTimeout(shrinkTimer);
    const apply = () => {
      lastAsked = { w, h };
      void Promise.resolve(api.resize(w, h)).catch(() => {});
    };
    if (w >= lastAsked.w && h >= lastAsked.h) apply();
    else shrinkTimer = setTimeout(apply, SHRINK_DELAY_MS);
  }

  /** Mide el contenido y, si cambió, ajusta isla y ventana. Se mide el tamaño de diseño (`offsetWidth/Height`): el de
   *  `getBoundingClientRect` incluye la escala de la animación de aparición (0,94) y dejaba la isla más estrecha de lo
   *  necesario, con el último botón cortado, hasta que algo cambiaba el contenido. */
  let measure = () => {};

  onMount(() => {
    if (!content) return;
    measure = () => {
      if (!content) return;
      // +2: el borde de la isla.
      const w = Math.ceil(content.offsetWidth) + 2;
      const h = Math.ceil(content.offsetHeight) + 2;
      if (w === width && h === height) return;
      width = w;
      height = h;
      resizeWindow(w, h);
    };
    measure();
    // La primera medida se pinta sin transición; las siguientes, con ella.
    requestAnimationFrame(() => (settled = true));
    const observer = typeof ResizeObserver === "function" ? new ResizeObserver(measure) : null;
    observer?.observe(content);
    return () => {
      observer?.disconnect();
      clearTimeout(shrinkTimer);
    };
  });

  const letter = (exe: string) => (exe.trim()[0] ?? "?").toUpperCase();
</script>

<div class="stage" class:closing={phase === "closing"} class:alt={openTick % 2 === 1} onanimationend={() => measure()}>
  <div class="island" class:settled style:width={width === null ? undefined : `${width}px`} style:height={height === null ? undefined : `${height}px`}>
    <GlassBackdrop kind="bar" blur={$theme.barBlur} tick={glassTick} anchored={{ gap: BOTTOM_MARGIN }} />
    <div class="tint" aria-hidden="true"></div>
    <DotField fixed={false} />
    <div class="content" bind:this={content}>
      {#if bgOpen && !powerOpen}
        <section class="bgpanel" aria-label="Aplicaciones en segundo plano">
          {#each background as a (a.exe)}
            <div class="card">
              <button class="main" title={backgroundLabel(a, formatBytes)} aria-label={backgroundLabel(a, formatBytes)} onclick={() => openBackground(a)}>
                {#await loadExeIcon(a.path, api.exeIcon)}
                  <span class="fallback">{letter(a.name)}</span>
                {:then url}
                  {#if url}<img src={url} alt="" draggable="false" />{:else}<span class="fallback">{letter(a.name)}</span>{/if}
                {/await}
                <span class="meta"><b>{a.name}</b><small>CPU {a.cpu_percent < 10 && a.cpu_percent > 0 ? a.cpu_percent.toFixed(1) : Math.round(a.cpu_percent)}% · {formatBytes(a.ram_bytes)}</small></span>
              </button>
              <button class="dots" aria-label={`Acciones de ${a.name}`} onclick={(e) => openActions(a, e)}>⋯</button>
            </div>
          {/each}
        </section>
      {/if}
      <nav class="row" aria-label="Aplicaciones abiertas por workspace">
        {#if !powerOpen}
          {#each groups as g, i (g.workspace)}
            {#if i > 0}<i class="sep" aria-hidden="true"></i>{/if}
            <div class="group" class:active={g.active} class:visible={g.visible} role="group" aria-label={`Workspace ${g.workspace}`}>
              <button class="ws" aria-label={`Ir al workspace ${g.workspace}`} aria-current={g.visible ? "true" : undefined} onclick={() => onWorkspace(g.workspace)}>{g.workspace}</button>
              {#each g.windows as w (w.id)}
                <button
                  class="app"
                  class:focused={w.focused}
                  class:minimized={w.minimized}
                  title={windowLabel(w)}
                  aria-label={`${windowLabel(w)}${w.minimized ? " (minimizada)" : ""}`}
                  onclick={() => open(w.id)}
                  transition:scale={{ start: 0.4, duration: 220 }}
                >
                  {#await loadIcon(w, api.icon)}
                    <span class="fallback">{letter(w.exe)}</span>
                  {:then url}
                    {#if url}<img src={url} alt="" draggable="false" />{:else}<span class="fallback">{letter(w.exe)}</span>{/if}
                  {/await}
                </button>
              {/each}
            </div>
          {/each}
          {#if background.length > 0}
            <i class="sep" aria-hidden="true"></i>
            <button class="bgtoggle" class:on={bgOpen} aria-expanded={bgOpen} aria-label={`Aplicaciones en segundo plano (${background.length})`} title="Aplicaciones en segundo plano" onclick={() => (bgOpen = !bgOpen)}>
              <span class="chev" aria-hidden="true"><Icon name="arrow-up" size={14} /></span>
              <span class="count">{background.length}</span>
            </button>
          {/if}
          {#if power}<i class="sep" aria-hidden="true"></i>{/if}
        {/if}
        {#if power}
          <PowerMenu allowChanges={power.allowChanges} onRun={power.run} {closeTick} bind:open={powerOpen} />
        {/if}
      </nav>
    </div>
  </div>
</div>

<style>
  :global(html, body) { margin: 0; height: 100%; background: transparent; overflow: hidden; }
  .stage { position: fixed; inset: 0; display: flex; justify-content: center; align-items: stretch; animation: rise 0.5s cubic-bezier(.22, 1, .36, 1) both; }
  /* Aparece subiendo desde el borde de abajo; al cerrarse, la misma animación al revés. */
  .stage.closing { animation: fall 0.38s cubic-bezier(.22, 1, .36, 1) both; pointer-events: none; }
  /* Dos nombres iguales para volver a reproducir la animación en cada despliegue. */
  .stage.alt { animation-name: rise-b; }
  .stage.alt.closing { animation-name: fall; }
  @keyframes fall { from { opacity: 1; transform: none; } to { opacity: 0; transform: translateY(26px) scale(0.94); } }
  @keyframes rise-b { from { opacity: 0; transform: translateY(26px) scale(0.94); } to { opacity: 1; transform: none; } }
  @keyframes rise { from { opacity: 0; transform: translateY(26px) scale(0.94); } to { opacity: 1; transform: none; } }
  .stage { align-items: flex-end; }
  .island {
    position: relative; box-sizing: border-box; overflow: hidden; border-radius: 22px;
    border: 1px solid var(--line, rgba(255, 255, 255, 0.14)); min-width: 40px; min-height: 48px;
  }
  .content { position: relative; z-index: 1; display: flex; flex-direction: column; width: max-content; }
  .island.settled { transition: width 0.38s cubic-bezier(.22, 1, .36, 1), height 0.38s cubic-bezier(.22, 1, .36, 1); }
  .tint { position: absolute; inset: 0; z-index: 0; pointer-events: none; background: rgba(10, 10, 11, var(--bar-alpha, 1)); }
  .row {
    position: relative; display: flex; align-items: center; justify-content: center; gap: 6px; height: 46px; flex: none;
    padding: 0 12px; box-sizing: border-box; color: var(--fg); font: 11px var(--mono);
  }
  .group { display: flex; align-items: center; gap: 4px; padding: 3px 4px 3px 3px; border-radius: 999px; transition: background 0.3s ease; }
  .group.visible { background: rgba(255, 255, 255, 0.07); }
  .ws {
    width: 22px; height: 22px; padding: 0; display: grid; place-items: center; flex: none; cursor: pointer;
    border-radius: 50%; border: 1px solid var(--line, rgba(255, 255, 255, 0.14)); background: transparent; color: var(--muted);
    font: 600 11px var(--mono); transition: background 0.3s ease, color 0.3s ease, border-color 0.3s ease, box-shadow 0.3s ease;
  }
  .ws:hover { color: var(--fg); border-color: var(--accent); }
  .group.active .ws { background: var(--accent); color: var(--on-accent); border-color: transparent; box-shadow: 0 0 12px -3px var(--accent); }
  .group.visible:not(.active) .ws { color: var(--fg); border-color: var(--accent); }
  .sep { width: 1px; height: 22px; background: var(--line, rgba(255, 255, 255, 0.14)); flex: none; }
  .app {
    position: relative; width: 32px; height: 32px; padding: 0; display: grid; place-items: center; flex: none; cursor: pointer;
    border: 0; border-radius: 10px; background: transparent;
    transition: transform 0.25s cubic-bezier(.3, 1.6, .5, 1), background 0.25s ease, opacity 0.25s ease;
  }
  .app:hover { transform: translateY(-2px) scale(1.1); background: rgba(255, 255, 255, 0.1); }
  .app:active { transform: scale(0.92); }
  .app img { width: 22px; height: 22px; object-fit: contain; animation: icon-in 0.3s ease both; pointer-events: none; }
  .fallback { width: 22px; height: 22px; display: grid; place-items: center; border-radius: 7px; background: rgba(255, 255, 255, 0.14); font: 600 12px var(--mono); }
  .bgtoggle {
    display: flex; align-items: center; gap: 5px; height: 28px; padding: 0 10px; flex: none; cursor: pointer; border-radius: 999px;
    border: 1px solid var(--line, rgba(255, 255, 255, 0.14)); background: transparent; color: var(--fg); font: 600 11px var(--mono);
    transition: background 0.3s ease, color 0.3s ease, border-color 0.3s ease;
  }
  .bgtoggle:hover { border-color: var(--accent); }
  .bgtoggle.on { background: var(--accent); color: var(--on-accent); border-color: transparent; }
  .bgtoggle .chev { display: grid; transition: transform 0.35s cubic-bezier(.22, 1, .36, 1); }
  .bgtoggle.on .chev { transform: rotate(180deg); }
  /* La lista sube sobre la barra: se despliega hacia arriba y no ensancha la isla. */
  .bgpanel {
    box-sizing: border-box; min-width: 360px; max-height: 264px; overflow-y: auto; padding: 10px 10px 4px; display: grid;
    grid-template-columns: repeat(2, minmax(170px, 1fr)); gap: 6px; scrollbar-width: none; animation: bg-in 0.3s cubic-bezier(.22, 1, .36, 1) both;
  }
  .bgpanel::-webkit-scrollbar { display: none; }
  @keyframes bg-in { from { opacity: 0; transform: translateY(10px); } to { opacity: 1; transform: none; } }
  .card {
    position: relative; display: flex; flex-wrap: wrap; align-items: center; gap: 4px; min-width: 0; padding: 4px 4px 4px 6px;
    border: 1px solid transparent; border-radius: 14px; background: rgba(255, 255, 255, 0.06); transition: background 0.25s ease, border-color 0.25s ease;
  }
  .card:hover { background: rgba(255, 255, 255, 0.1); border-color: var(--accent); }
  .card .main {
    display: flex; flex: 1; align-items: center; gap: 8px; min-width: 0; padding: 2px; cursor: pointer; text-align: left; color: var(--fg);
    border: 0; background: transparent; font: 11px var(--mono);
  }
  .card img, .card .fallback { width: 24px; height: 24px; flex: none; object-fit: contain; }
  .card .meta { display: flex; flex-direction: column; min-width: 0; }
  .card b { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .card small { color: var(--muted); font-size: 10px; white-space: nowrap; }
  .dots { width: 26px; height: 26px; padding: 0; flex: none; cursor: pointer; border: 0; border-radius: 50%; background: transparent; color: var(--muted); font: 700 15px var(--mono); line-height: 1; }
  .dots:hover { background: rgba(255, 255, 255, 0.14); color: var(--fg); }
  .app.minimized { opacity: 0.5; }
  /* La ventana con el foco: un puntito de acento debajo y un fondo suave. */
  .app.focused { background: rgba(255, 255, 255, 0.12); }
  .app.focused::after {
    content: ""; position: absolute; left: 50%; bottom: 1px; width: 12px; height: 3px; margin-left: -6px; border-radius: 3px;
    background: var(--accent); box-shadow: 0 0 8px var(--accent);
  }
  @keyframes icon-in { from { opacity: 0; transform: scale(0.7); } }
  @media (prefers-reduced-motion: reduce) { .island.settled, .app, .group, .ws { transition: none; } .app img { animation: none; } .stage, .stage.closing { animation: none; } }
</style>
