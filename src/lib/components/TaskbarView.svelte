<!-- Barra de tareas de Tesela: una isla centrada abajo con las ventanas de cada workspace (1, 2, 3…), cada una con su
     icono. Su ancho sigue a su contenido y cambia con una transición suave; la ventana se ajusta al mismo ancho. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { scale } from "svelte/transition";
  import type { StateSnapshot } from "../api/state";
  import type { TaskbarApi } from "../api/shell";
  import { groupsFor, loadIcon, windowLabel } from "../stores/taskbar";
  import { theme } from "../stores/theme";
  import GlassBackdrop from "./GlassBackdrop.svelte";

  let {
    state: desktop,
    monitorId,
    api,
    onWorkspace,
    openTick = 0,
    closeTick = 0,
  }: {
    state: StateSnapshot | null;
    monitorId: number;
    api: TaskbarApi;
    onWorkspace: (n: number) => void;
    /** Sube cada vez que el núcleo despliega la barra: se reproduce la animación de aparición. */
    openTick?: number;
    /** Sube cuando el núcleo la cierra: se reproduce la animación al revés. */
    closeTick?: number;
  } = $props();

  const SHRINK_DELAY_MS = 380;

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

  /** Tocar una app la enfoca y cierra la barra. */
  function open(id: number) {
    void Promise.resolve(api.focus(id)).catch(() => {});
    void Promise.resolve(api.hide()).catch(() => {});
  }

  let row: HTMLElement | undefined = $state();
  /** Ancho de la isla; cambia con transición. */
  let width: number | null = $state(null);
  let settled = $state(false);
  /** Sube cuando la ventana cambió de tamaño: el cristal vuelve a copiar lo de detrás. */
  let glassTick = $state(0);
  let shrinkTimer: ReturnType<typeof setTimeout> | undefined;
  let glassTimer: ReturnType<typeof setTimeout> | undefined;
  let lastAsked = 0;

  /** Pide a la ventana el ancho nuevo: si crece, ya; si encoge, al terminar la transición de la isla. */
  function resizeWindow(target: number) {
    clearTimeout(shrinkTimer);
    const apply = () => {
      lastAsked = target;
      void Promise.resolve(api.resize(target)).catch(() => {});
      clearTimeout(glassTimer);
      glassTimer = setTimeout(() => glassTick++, 250);
    };
    if (target >= lastAsked) apply();
    else shrinkTimer = setTimeout(apply, SHRINK_DELAY_MS);
  }

  onMount(() => {
    if (!row) return;
    const measure = () => {
      if (!row) return;
      const target = Math.ceil(row.getBoundingClientRect().width);
      if (target === width) return;
      width = target;
      resizeWindow(target);
    };
    measure();
    // La primera medida se pinta sin transición; las siguientes, con ella.
    requestAnimationFrame(() => (settled = true));
    const observer = typeof ResizeObserver === "function" ? new ResizeObserver(measure) : null;
    observer?.observe(row);
    return () => {
      observer?.disconnect();
      clearTimeout(shrinkTimer);
      clearTimeout(glassTimer);
    };
  });

  const letter = (exe: string) => (exe.trim()[0] ?? "?").toUpperCase();
</script>

<div class="stage" class:closing={phase === "closing"} class:alt={openTick % 2 === 1}>
  <div class="island" class:settled style:width={width === null ? undefined : `${width}px`}>
    <GlassBackdrop kind="bar" blur={$theme.barBlur} tick={glassTick} />
    <div class="tint" aria-hidden="true"></div>
    <nav class="row" aria-label="Aplicaciones abiertas por workspace" bind:this={row}>
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
    </nav>
  </div>
</div>

<style>
  :global(html, body) { margin: 0; height: 100%; background: transparent; overflow: hidden; }
  .stage { position: fixed; inset: 0; display: flex; justify-content: center; align-items: stretch; animation: rise 0.5s cubic-bezier(.22, 1, .36, 1) both; }
  /* Aparece subiendo desde el borde de abajo; al cerrarse, la misma animación al revés. */
  .stage.closing { animation: rise 0.38s cubic-bezier(.22, 1, .36, 1) reverse both; pointer-events: none; }
  /* Dos nombres iguales para volver a reproducir la animación en cada despliegue. */
  .stage.alt { animation-name: rise-b; }
  .stage.alt.closing { animation-name: rise-b; }
  @keyframes rise-b { from { opacity: 0; transform: translateY(26px) scale(0.94); } to { opacity: 1; transform: none; } }
  @keyframes rise { from { opacity: 0; transform: translateY(26px) scale(0.94); } to { opacity: 1; transform: none; } }
  .island {
    position: relative; box-sizing: border-box; height: 100%; overflow: hidden; border-radius: 999px;
    border: 1px solid var(--line, rgba(255, 255, 255, 0.14)); min-width: 40px;
  }
  .island.settled { transition: width 0.38s cubic-bezier(.22, 1, .36, 1); }
  .tint { position: absolute; inset: 0; z-index: 0; pointer-events: none; background: rgba(10, 10, 11, var(--bar-alpha, 1)); }
  .row {
    position: relative; z-index: 1; display: flex; align-items: center; gap: 6px; width: max-content; height: 100%;
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
