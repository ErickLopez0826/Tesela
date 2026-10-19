<!-- Ventana emergente de una aplicación en segundo plano: sus acciones en una lista (silenciar, cerrar, las propias de la app).
     No toma el foco; el núcleo la coloca junto al botón «⋯», la mide por su contenido y la cierra al actuar o al hacer clic fuera. -->
<script lang="ts">
  import { onMount } from "svelte";
  import type { AppAction, PopupApi, PopupTarget } from "../api/shell";
  import { loadExeIcon } from "../stores/taskbar";

  let {
    target,
    api,
    openTick = 0,
    closeTick = 0,
  }: {
    target: PopupTarget | null;
    api: PopupApi;
    /** Sube cada vez que el núcleo la abre: se reproduce la animación de aparición. */
    openTick?: number;
    /** Sube cuando el núcleo empieza a cerrarla. */
    closeTick?: number;
  } = $props();

  let actions: AppAction[] = $state([]);
  let loading = $state(true);
  let confirming: string | null = $state(null);
  let error: string | null = $state(null);
  let phase: "open" | "closing" = $state("open");
  let box: HTMLElement | undefined = $state();
  let seenOpen = 0;
  let seenClose = 0;

  async function load(exe: string) {
    loading = true;
    try {
      const list = await api.actions(exe);
      if (target?.exe === exe) actions = list;
    } catch (e) {
      if (target?.exe === exe) error = String(e);
    } finally {
      if (target?.exe === exe) loading = false;
    }
  }

  $effect(() => {
    if (openTick !== seenOpen) {
      seenOpen = openTick;
      phase = "open";
    }
    if (closeTick !== seenClose) {
      seenClose = closeTick;
      phase = "closing";
    }
  });

  // Cada vez que se abre para una app (aunque sea la misma) se vuelven a pedir sus acciones: el estado puede haber cambiado.
  $effect(() => {
    void openTick;
    if (!target) return;
    actions = [];
    confirming = null;
    error = null;
    void load(target.exe);
  });

  async function run(action: AppAction) {
    if (!target) return;
    if (action.danger && confirming !== action.id) {
      confirming = action.id;
      return;
    }
    confirming = null;
    error = null;
    try {
      await api.run(target.exe, action.id);
      if (action.on === null) {
        // Una acción puntual: hecho, la ventana se cierra.
        void Promise.resolve(api.close()).catch(() => {});
      } else {
        // Un interruptor sigue abierto y muestra su estado nuevo.
        actions = await api.actions(target.exe).catch(() => actions);
      }
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => {
    if (!box) return;
    let last = "";
    const measure = () => {
      if (!box) return;
      const w = Math.ceil(box.offsetWidth);
      const h = Math.ceil(box.offsetHeight);
      if (`${w}x${h}` === last) return;
      last = `${w}x${h}`;
      void Promise.resolve(api.resize(w, h)).catch(() => {});
    };
    measure();
    const observer = typeof ResizeObserver === "function" ? new ResizeObserver(measure) : null;
    observer?.observe(box);
    return () => observer?.disconnect();
  });

  const letter = (name: string) => (name.trim()[0] ?? "?").toUpperCase();
</script>

<div class="stage" class:closing={phase === "closing"} class:alt={openTick % 2 === 1}>
  <div class="box" bind:this={box} role="menu" aria-label={target ? `Acciones de ${target.name}` : "Acciones"}>
    {#if target}
      <header>
        {#await loadExeIcon(target.path, api.exeIcon)}
          <span class="fallback">{letter(target.name)}</span>
        {:then url}
          {#if url}<img src={url} alt="" draggable="false" />{:else}<span class="fallback">{letter(target.name)}</span>{/if}
        {/await}
        <b>{target.name}</b>
      </header>
      <div class="list">
        {#each actions as action (action.id)}
          {#if action.danger && confirming === action.id}
            <div class="ask" role="group">
              <span>¿Cerrar {target.name}?</span>
              <button class="item danger" role="menuitem" onclick={() => run(action)}>Sí, cerrar</button>
              <button class="item" role="menuitem" onclick={() => (confirming = null)}>No</button>
            </div>
          {:else if !confirming}
            <button class="item" class:danger={action.danger} class:on={action.on === true} role="menuitem" aria-checked={action.on === null ? undefined : action.on} onclick={() => run(action)}>
              <span>{action.label}</span>
              {#if action.on !== null}<i class="dot" aria-hidden="true"></i>{/if}
            </button>
          {/if}
        {/each}
        {#if loading && actions.length === 0}<span class="hint">Cargando…</span>{/if}
        {#if error}<span class="err" role="alert">{error}</span>{/if}
      </div>
    {/if}
  </div>
</div>

<style>
  :global(html, body) { margin: 0; height: 100%; background: transparent; overflow: hidden; }
  .stage { position: fixed; inset: 0; display: flex; align-items: flex-start; justify-content: flex-end; animation: pop-in 0.5s cubic-bezier(.22, 1, .36, 1) both; }
  .stage.alt { animation-name: pop-in-b; }
  /* Se cierra con su propia animación (con el nombre de la de apertura no se reiniciaría) y en el mismo tiempo que el panel y la barra. */
  .stage.closing, .stage.alt.closing { animation: pop-out 0.38s cubic-bezier(.22, 1, .36, 1) both; pointer-events: none; }
  @keyframes pop-in { from { opacity: 0; transform: translateY(-8px) scale(0.96); } to { opacity: 1; transform: none; } }
  @keyframes pop-in-b { from { opacity: 0; transform: translateY(-8px) scale(0.96); } to { opacity: 1; transform: none; } }
  @keyframes pop-out { from { opacity: 1; transform: none; } to { opacity: 0; transform: translateY(-8px) scale(0.96); } }
  .box {
    box-sizing: border-box; width: max-content; min-width: 200px; max-width: 340px; padding: 8px; color: var(--fg); font: 11px var(--mono);
    border: 1px solid var(--line, rgba(255, 255, 255, 0.14)); border-radius: 18px; background: rgba(12, 12, 14, 0.96);
  }
  header { display: flex; align-items: center; gap: 8px; padding: 4px 6px 8px; border-bottom: 1px solid var(--line, rgba(255, 255, 255, 0.14)); margin-bottom: 6px; }
  header img, header .fallback { width: 22px; height: 22px; flex: none; object-fit: contain; }
  .fallback { display: grid; place-items: center; border-radius: 7px; background: rgba(255, 255, 255, 0.14); font: 600 12px var(--mono); }
  header b { font-weight: 600; letter-spacing: 0.04em; text-transform: uppercase; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .list { display: flex; flex-direction: column; gap: 2px; }
  .item {
    display: flex; align-items: center; justify-content: space-between; gap: 10px; min-height: 28px; padding: 0 10px; cursor: pointer; text-align: left;
    color: var(--fg); background: transparent; border: 0; border-radius: 10px; font: 600 11px var(--mono); letter-spacing: 0.03em; white-space: nowrap;
    transition: background 0.2s ease, color 0.2s ease;
  }
  .item:hover { background: rgba(255, 255, 255, 0.12); }
  .item.on { color: var(--accent); }
  .item.danger { color: #ff7a7f; }
  .item.danger:hover { background: #ff5a5f; color: #000; }
  .dot { width: 8px; height: 8px; border-radius: 50%; border: 1px solid currentColor; }
  .item.on .dot { background: currentColor; }
  .ask { display: flex; flex-direction: column; gap: 2px; padding: 2px; }
  .ask span { padding: 4px 10px 6px; font: 600 11px var(--mono); }
  .hint { padding: 6px 10px; color: var(--muted); }
  .err { padding: 4px 10px; color: #ff7a7f; font-size: 10px; }
  @media (prefers-reduced-motion: reduce) { .stage, .stage.closing { animation: none; } }
</style>
