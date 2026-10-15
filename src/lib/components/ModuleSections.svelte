<script lang="ts">
  import { MODULE_ACTION_EVENT, type InstalledModule } from "../api/modules";
  import ModuleFrame from "./ModuleFrame.svelte";

  let { modules }: { modules: InstalledModule[] } = $props();

  const withPanel = $derived(modules.filter((m) => m.enabled && m.manifest.ui.panel));

  function runAction(module: string, action: string) {
    window.dispatchEvent(new CustomEvent(MODULE_ACTION_EVENT, { detail: { module, action } }));
  }
</script>

{#each withPanel as m (m.manifest.id)}
  <section class="section module-section">
    <h3>{m.manifest.ui.panel?.title}</h3>
    <ModuleFrame module={m} height={m.manifest.ui.panel?.height ?? 160} />
    {#if m.manifest.actions.length > 0}
      <div class="buttons">
        {#each m.manifest.actions as a (a.id)}
          <button class="btn" onclick={() => runAction(m.manifest.id, a.id)}>{a.title}</button>
        {/each}
      </div>
    {/if}
  </section>
{/each}
