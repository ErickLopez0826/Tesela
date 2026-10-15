<script lang="ts">
  import { onMount } from "svelte";
  import ExclusionPanel from "./lib/components/ExclusionPanel.svelte";
  import WindowList from "./lib/components/WindowList.svelte";
  import { exclusionsStore } from "./lib/stores/exclusions";
  import { windowsStore } from "./lib/stores/windows";

  let tab: "windows" | "exclusions" = $state("windows");

  onMount(() => {
    windowsStore.refresh();
    exclusionsStore.refresh();
  });
</script>

<main>
  <nav>
    <button class:active={tab === "windows"} onclick={() => (tab = "windows")}>Ventanas</button>
    <button class:active={tab === "exclusions"} onclick={() => (tab = "exclusions")}>Exclusiones</button>
    <button onclick={() => windowsStore.refresh()} disabled={$windowsStore.loading}>Refrescar</button>
  </nav>

  {#if $windowsStore.error}
    <p>Error: {$windowsStore.error}</p>
  {/if}
  {#if $exclusionsStore.error}
    <p>Error: {$exclusionsStore.error}</p>
  {/if}

  {#if tab === "windows"}
    <h1>Ventanas visibles</h1>
    <WindowList windows={$windowsStore.windows} />
  {:else}
    <h1>Exclusiones</h1>
    <ExclusionPanel
      windows={$windowsStore.windows}
      rules={$exclusionsStore.rules}
      onAdd={(exe, title) => exclusionsStore.add(exe, title)}
      onRemove={(r) => exclusionsStore.remove(r.exe, r.title_contains)}
    />
  {/if}
</main>

<style>
  main { font-family: system-ui, sans-serif; padding: 1rem; }
  nav { display: flex; gap: 0.5rem; margin-bottom: 1rem; }
  nav .active { font-weight: bold; text-decoration: underline; }
</style>
