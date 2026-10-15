<script lang="ts">
  import type { ActionOutcome, PowerActionName } from "../../api/system";

  let {
    allowChanges,
    onRun,
  }: {
    allowChanges: boolean;
    onRun: (action: PowerActionName) => Promise<ActionOutcome>;
  } = $props();

  const actions: { name: PowerActionName; label: string; confirm: string }[] = [
    { name: "shutdown", label: "Apagar", confirm: "¿Apagar el equipo?" },
    { name: "suspend", label: "Suspender", confirm: "¿Suspender el equipo?" },
    { name: "restart", label: "Reiniciar", confirm: "¿Reiniciar el equipo?" },
    { name: "hibernate", label: "Hibernar", confirm: "¿Hibernar el equipo?" },
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
    <div class="notice">
      {actions.find((a) => a.name === pending)?.confirm}
      <div class="buttons ask">
        <button class="btn danger" onclick={() => confirm(pending!)}>Sí</button>
        <button class="btn" onclick={() => (pending = null)}>Cancelar</button>
      </div>
    </div>
  {:else}
    <div class="buttons">
      {#each actions as a (a.name)}
        <button class="btn" onclick={() => (pending = a.name)}>{a.label}</button>
      {/each}
    </div>
  {/if}
  {#if outcome}<div class="notice" class:simulated={outcome.simulated}>{outcome.description}</div>{/if}
  {#if error}<div class="notice error">{error}</div>{/if}
</section>

<style>
  .ask { margin-top: 6px; }
</style>
