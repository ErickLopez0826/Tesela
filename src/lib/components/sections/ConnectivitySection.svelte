<script lang="ts">
  import type { ActionOutcome, RadioKindName, RadioState } from "../../api/system";

  let {
    radios,
    allowChanges,
    onSet,
  }: {
    radios: RadioState[];
    allowChanges: boolean;
    onSet: (kind: RadioKindName, on: boolean) => Promise<ActionOutcome>;
  } = $props();

  const labels: Record<RadioKindName, string> = { wifi: "Wi-Fi", bluetooth: "Bluetooth", hotspot: "Compartir red" };

  let outcome: ActionOutcome | null = $state(null);
  let error: string | null = $state(null);

  async function toggle(r: RadioState) {
    error = null;
    try {
      outcome = await onSet(r.kind, !r.on);
    } catch (e) {
      outcome = null;
      error = String(e);
    }
  }
</script>

<section class="section">
  <h3>Conectividad {#if !allowChanges}<span class="badge safe">modo seguro: simulado</span>{/if}</h3>
  <div class="buttons">
    {#each radios as r (r.kind)}
      <button class="btn" class:on={r.on} disabled={!r.available} aria-pressed={r.on} onclick={() => toggle(r)}>
        {labels[r.kind]}: {r.available ? (r.on ? "sí" : "no") : "no disponible"}
      </button>
    {/each}
  </div>
  {#if outcome}<div class="notice" class:simulated={outcome.simulated}>{outcome.description}</div>{/if}
  {#if error}<div class="notice error">{error}</div>{/if}
</section>
