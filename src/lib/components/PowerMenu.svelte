<!-- Energía en la barra de tareas: un botón que, al pulsarlo, despliega en la misma fila las acciones con su icono y su
     texto. Elegir una pide confirmación ahí mismo; el menú se pliega solo si no se usa. -->
<script lang="ts">
  import { onDestroy } from "svelte";
  import Icon, { type IconName } from "./Icon.svelte";
  import type { ActionOutcome, PowerActionName } from "../api/system";

  let {
    allowChanges,
    onRun,
    /** Sube cuando la barra se cierra: el menú se pliega. */
    closeTick = 0,
    /** El menú está abierto; la barra, mientras tanto, solo muestra sus botones. */
    open = $bindable(false),
  }: {
    allowChanges: boolean;
    onRun: (action: PowerActionName) => Promise<ActionOutcome>;
    closeTick?: number;
    open?: boolean;
  } = $props();

  const actions: { name: PowerActionName; label: string; confirm: string; icon: IconName }[] = [
    { name: "suspend", label: "Suspender", confirm: "¿Suspender?", icon: "moon" },
    { name: "restart", label: "Reiniciar", confirm: "¿Reiniciar?", icon: "refresh" },
    { name: "hibernate", label: "Hibernar", confirm: "¿Hibernar?", icon: "snowflake" },
    { name: "shutdown", label: "Apagar", confirm: "¿Apagar?", icon: "power" },
  ];
  const FOLD_MS = 10000;

  let pending: PowerActionName | null = $state(null);
  let note: string | null = $state(null);
  let failed = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  function arm() {
    clearTimeout(timer);
    timer = setTimeout(fold, FOLD_MS);
  }
  function fold() {
    clearTimeout(timer);
    open = false;
    pending = null;
  }
  function toggle() {
    if (open) return fold();
    open = true;
    note = null;
    arm();
  }
  async function run(action: PowerActionName) {
    pending = null;
    failed = false;
    try {
      note = (await onRun(action)).description;
    } catch (e) {
      failed = true;
      note = String(e);
    }
    arm();
  }

  let seenClose = 0;
  $effect(() => {
    if (closeTick !== seenClose) {
      seenClose = closeTick;
      fold();
      note = null;
    }
  });
  onDestroy(() => clearTimeout(timer));
  const asked = $derived(actions.find((a) => a.name === pending));
</script>

<div class="power" class:open>
  {#if !open}
  <button class="toggle" class:on={open} aria-label="Energía" aria-expanded={open} title={allowChanges ? "Energía" : "Energía (modo seguro: simulado)"} onclick={toggle}>
    <Icon name="power" size={18} />
  </button>
  {:else}
    <div class="menu" role="group" aria-label="Acciones de energía">
      <button class="act back" aria-label="Volver a la barra de tareas" onclick={fold}>
        <Icon name="arrow-up" size={14} /><span>Volver</span>
      </button>
      {#if asked}
        <span class="question">{asked.confirm}</span>
        <button class="act danger" onclick={() => run(asked.name)} onpointerenter={arm}>Sí</button>
        <button class="act" onclick={() => { pending = null; arm(); }}>Cancelar</button>
      {:else}
        {#each actions as a (a.name)}
          <button class="act" class:danger={a.name === "shutdown"} onclick={() => { pending = a.name; arm(); }}>
            <Icon name={a.icon} size={16} /><span>{a.label}</span>
          </button>
        {/each}
        {#if !allowChanges}<span class="safe">simulado</span>{/if}
      {/if}
    </div>
  {/if}
  {#if note}<span class="note" class:error={failed} role="status">{note}</span>{/if}
</div>

<style>
  .power { display: flex; align-items: center; gap: 4px; flex: none; }
  .toggle {
    width: 28px; height: 28px; padding: 0; display: grid; place-items: center; cursor: pointer; flex: none;
    border-radius: 50%; border: 1px solid var(--line, rgba(255, 255, 255, 0.14)); background: transparent; color: var(--fg);
    transition: background 0.3s ease, color 0.3s ease, border-color 0.3s ease, transform 0.25s cubic-bezier(.3, 1.6, .5, 1);
  }
  .toggle:hover { border-color: var(--accent); transform: scale(1.08); }
  .toggle.on { background: var(--accent); color: var(--on-accent); border-color: transparent; }
  .menu { display: flex; align-items: center; gap: 2px; padding: 2px; border-radius: 999px; background: rgba(255, 255, 255, 0.07); animation: unfold 0.28s cubic-bezier(.22, 1, .36, 1) both; }
  @keyframes unfold { from { opacity: 0; transform: translateX(-8px) scale(0.92); } to { opacity: 1; transform: none; } }
  .act {
    display: flex; align-items: center; gap: 5px; height: 26px; padding: 0 10px; cursor: pointer; border: 0; border-radius: 999px;
    background: transparent; color: var(--fg); font: 600 10px var(--mono); letter-spacing: 0.06em; text-transform: uppercase; white-space: nowrap;
    transition: background 0.25s ease, color 0.25s ease;
  }
  .act:hover { background: rgba(255, 255, 255, 0.14); }
  .act.back :global(svg) { transform: rotate(-90deg); }
  .act.danger:hover { background: var(--accent); color: var(--on-accent); }
  .question { padding: 0 8px; font: 600 11px var(--mono); white-space: nowrap; }
  .safe { padding: 0 6px; font: 9px var(--mono); color: var(--muted); text-transform: uppercase; }
  .note { max-width: 220px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font: 10px var(--mono); color: var(--muted); }
  .note.error { color: #ff5a5f; }
  @media (prefers-reduced-motion: reduce) { .menu { animation: none; } .toggle { transition: none; } }
</style>
