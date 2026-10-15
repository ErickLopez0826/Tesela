<script lang="ts">
  import { onMount } from "svelte";
  import WindowList from "./lib/components/WindowList.svelte";
  import { windowsStore } from "./lib/stores/windows";

  onMount(() => windowsStore.refresh());
</script>

<main>
  <h1>Ventanas visibles</h1>
  <button onclick={() => windowsStore.refresh()} disabled={$windowsStore.loading}>Refrescar</button>
  {#if $windowsStore.error}
    <p>Error: {$windowsStore.error}</p>
  {/if}
  <WindowList windows={$windowsStore.windows} />
</main>

<style>
  main { font-family: system-ui, sans-serif; padding: 1rem; }
</style>
