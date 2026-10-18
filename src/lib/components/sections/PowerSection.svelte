<script lang="ts">
  import Icon, { type IconName } from "../Icon.svelte";
  import type { ActionOutcome, PowerActionName } from "../../api/system";

  let {
    allowChanges,
    onRun,
  }: {
    allowChanges: boolean;
    onRun: (action: PowerActionName) => Promise<ActionOutcome>;
  } = $props();

  const actions: { name: PowerActionName; label: string; confirm: string; icon: IconName }[] = [
    { name: "suspend", label: "Suspender", confirm: "¿Suspender el equipo?", icon: "moon" },
    { name: "restart", label: "Reiniciar", confirm: "¿Reiniciar el equipo?", icon: "refresh" },
    { name: "hibernate", label: "Hibernar", confirm: "¿Hibernar el equipo?", icon: "snowflake" },
    { name: "shutdown", label: "Apagar", confirm: "¿Apagar el equipo?", icon: "power" },
  ];

  let pending: PowerActionName | null = $state(null);
  let outcome: ActionOutcome | null = $state(null);
  let error: string | null = $state(null);

  async function confirm(action: PowerActionName) {
    pending = null;
    error = null;
    try {
      outcome = await onRun(action);
    } catch (e) {
      outcome = null;
      error = String(e);
    }
  }
</script>

<section class="section">
  <h3>Energía {#if !allowChanges}<span class="badge safe">modo seguro: simulado</span>{/if}</h3>
  {#if pending}
    <div class="ask">
      <p class="question">{actions.find((a) => a.name === pending)?.confirm}</p>
      <div class="answers">
        <button class="btn danger big" onclick={() => confirm(pending!)}>Sí</button>
        <button class="btn big" onclick={() => (pending = null)}>Cancelar</button>
      </div>
    </div>
  {:else}
    <div class="tiles">
      {#each actions as a (a.name)}
        <button class="tile" class:danger={a.name === "shutdown"} onclick={() => (pending = a.name)}>
          <Icon name={a.icon} size={22} />
          <span>{a.label}</span>
        </button>
      {/each}
    </div>
  {/if}
  {#if outcome}<div class="notice" class:simulated={outcome.simulated}>{outcome.description}</div>{/if}
  {#if error}<div class="notice error">{error}</div>{/if}
</section>

<style>
  /* Confirmación: la pregunta y dos botones grandes, centrados. */
  .ask { display: flex; flex-direction: column; align-items: center; gap: 16px; padding: 18px 12px; text-align: center; }
  .question { margin: 0; font: 600 18px "Segoe UI", system-ui, sans-serif; }
  .answers { display: flex; justify-content: center; gap: 14px; width: 100%; }
  .big { flex: 0 1 190px; min-height: 52px; padding: 0 22px; border-radius: 16px; font-size: 16px; font-weight: 600; }
  .tiles { display: grid; grid-template-columns: repeat(4, 1fr); gap: 10px; }
  .tile { display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 16px 8px; border-radius: 20px; color: var(--fg); background: rgba(255, 255, 255, 0.05); border: 1px solid var(--line); font: 600 11px var(--mono); letter-spacing: 0.08em; text-transform: uppercase; cursor: pointer; transition: background 0.35s, border-color 0.35s, box-shadow 0.35s, transform 0.35s cubic-bezier(.3, 1.6, .5, 1); }
  .tile:hover { transform: translateY(-2px); border-color: var(--accent); background: rgba(255, 255, 255, 0.1); box-shadow: 0 8px 22px -12px rgba(255, 255, 255, 0.7); }
  .tile.danger:hover { background: var(--accent); color: var(--on-accent); }
</style>
