<script lang="ts">
  import type { WindowInfo } from "../api/windows";

  let { windows }: { windows: WindowInfo[] } = $props();
</script>

<table>
  <thead>
    <tr><th>Ejecutable</th><th>Título</th></tr>
  </thead>
  <tbody>
    {#each windows as w (w.handle)}
      <tr><td>{w.exe}</td><td>{w.title}</td></tr>
    {/each}
  </tbody>
</table>

<style>
  table { border-collapse: collapse; width: 100%; }
  th, td { text-align: left; padding: 4px 8px; border-bottom: 1px solid #8884; }
</style>
