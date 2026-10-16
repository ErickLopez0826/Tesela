<script lang="ts">
  import type { WorkspaceSnapshot } from "../api/state";

  let { workspaces, onWorkspace }: { workspaces: WorkspaceSnapshot[]; onWorkspace: (number: number) => void } = $props();

  // --- Animaciones de los workspaces -------------------------------------------------
  const FLIP_MS = 380;
  const ENTER_MS = 800;
  let zone: HTMLElement | undefined = $state();
  let pill = $state({ left: 0, right: 0, ready: false });
  let fresh = $state<Set<number>>(new Set());
  let known: Set<number> | null = null;
  const before = new Map<number, number>();
  let freshTimer: ReturnType<typeof setTimeout> | undefined;

  const buttons = () => (zone ? Array.from(zone.querySelectorAll<HTMLElement>("[data-ws]")) : []);

  // Antes de que cambie el DOM: dónde está cada botón.
  $effect.pre(() => {
    void workspaces;
    before.clear();
    for (const el of buttons()) before.set(Number(el.dataset.ws), el.getBoundingClientRect().left);
  });

  // Después: los nuevos entran desde arriba y los demás se desplazan hasta su sitio nuevo.
  $effect(() => {
    const numbers = workspaces.map((w) => w.number);
    if (known === null) {
      if (numbers.length > 0) known = new Set(numbers);
    } else {
      const added = numbers.filter((n) => !known!.has(n));
      known = new Set(numbers);
      if (added.length > 0) {
        fresh = new Set(added);
        clearTimeout(freshTimer);
        freshTimer = setTimeout(() => (fresh = new Set()), ENTER_MS);
      }
      for (const el of buttons()) {
        const old = before.get(Number(el.dataset.ws));
        if (old === undefined) continue;
        const dx = old - el.getBoundingClientRect().left;
        if (Math.abs(dx) < 0.5) continue;
        el.animate?.([{ transform: `translateX(${dx}px)` }, { transform: "none" }], { duration: FLIP_MS, easing: "cubic-bezier(.22, 1, .36, 1)" });
      }
    }
    // La pastilla va al botón activo; offsetLeft ya es la posición final aunque los vecinos aún se muevan.
    const active = zone?.querySelector<HTMLElement>(".ws.active");
    pill = active && zone
      ? { left: active.offsetLeft, right: zone.clientWidth - (active.offsetLeft + active.offsetWidth), ready: true }
      : { left: 0, right: 0, ready: false };
  });

  $effect(() => () => clearTimeout(freshTimer));
</script>

<div class="strip" bind:this={zone}>
    <span class="pill" class:ready={pill.ready} style:left={`${pill.left}px`} style:right={`${pill.right}px`} aria-hidden="true"></span>
    {#each workspaces as w (w.number)}
      <button
        class="ws"
        data-ws={w.number}
        class:visible={w.visible}
        class:active={w.active}
        class:busy={w.window_count > 0}
        class:enter={fresh.has(w.number)}
        onclick={() => onWorkspace(w.number)}
      >
        {w.number}
      </button>
    {/each}
</div>

<style>
  .strip { position: relative; display: flex; align-items: center; gap: 8px; }
  button { font: inherit; color: inherit; background: transparent; border: 0; cursor: pointer; }
  .ws { position: relative; z-index: 1; min-width: 24px; height: 24px; opacity: 0.45; }
  .ws.busy { opacity: 0.8; }
  .ws { border-radius: 50%; transition: background 0.4s ease, opacity 0.3s, color 0.3s; }
  .ws.visible { opacity: 1; background: rgba(255, 255, 255, 0.14); }
  .ws.active { background: transparent; color: var(--ws-on, #0a0a0b); font-weight: 700; }
  .ws.enter { animation: drop 0.8s cubic-bezier(.3, 1.3, .4, 1) both; }
  @keyframes drop {
    0% { transform: translateY(-34px) scale(0.25); opacity: 0; }
    55% { transform: translateY(3px) scale(1.08); opacity: 1; }
    100% { transform: none; }
  }
  /* Pastilla del workspace actual: se desliza de uno a otro como la pestaña del panel. */
  .pill {
    position: absolute; top: 50%; height: 24px; margin-top: -12px; border-radius: 999px; z-index: 0; opacity: 0; pointer-events: none;
    background: var(--ws, #f2f2f2); box-shadow: 0 0 14px color-mix(in srgb, var(--ws, #f2f2f2) 55%, transparent);
    transition: left 0.5s cubic-bezier(.3, 1.25, .35, 1), right 0.34s cubic-bezier(.3, 1.1, .35, 1), opacity 0.2s;
  }
  .pill.ready { opacity: 1; }
  @media (prefers-reduced-motion: reduce) { .ws.enter { animation: none; } .pill { transition: none; } }
  .ws:hover { background: rgba(255, 255, 255, 0.14); }
</style>
