<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { modulesApi, type InstalledModule } from "../api/modules";
  import type { BridgeRecord } from "../modules/bridge";
  import ModuleFrame from "../components/ModuleFrame.svelte";

  let { moduleId }: { moduleId: string } = $props();

  let module: InstalledModule | null = $state(null);
  let records: BridgeRecord[] = $state([]);
  let reported = false;
  let loadedSrc: string | null = $state(null);

  async function report(reason: string) {
    if (reported) return;
    reported = true;
    await invoke("selftest_report", { report: JSON.stringify({ module: moduleId, reason, records, loadedSrc, page: window.location.href }) });
  }

  onMount(async () => {
    const all = await modulesApi.list();
    module = all.find((m) => m.manifest.id === moduleId) ?? null;
    if (!module) return report("module_not_found");
    // Si el módulo no avisa de que terminó, se informa igualmente con lo observado.
    setTimeout(() => report("timeout"), 20000);
  });

  function onRecord(r: BridgeRecord) {
    records = [...records, r];
    // `probe.done` es la señal de fin de la sonda; el módulo de ejemplo se da por terminado tras un rato.
    if (r.method === "probe.done") setTimeout(() => report("done"), 300);
    if (r.method === "events.subscribe" && moduleId !== "probe") setTimeout(() => report("settled"), 2500);
  }
</script>

<main>
  <h1>Prueba del módulo {moduleId}</h1>
  {#if module}
    <ModuleFrame {module} {onRecord} onLoad={(s) => (loadedSrc = s)} />
  {/if}
  <ul>
    {#each records as r}
      <li>{r.method}: {r.ok ? "ok" : r.error}</li>
    {/each}
  </ul>
</main>
