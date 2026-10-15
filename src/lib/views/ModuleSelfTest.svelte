<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { MODULE_ACTION_EVENT, modulesApi, type InstalledModule } from "../api/modules";
  import type { BridgeRecord } from "../modules/bridge";
  import ModuleFrame from "../components/ModuleFrame.svelte";

  let { moduleId }: { moduleId: string } = $props();

  let module: InstalledModule | null = $state(null);
  let records: BridgeRecord[] = $state([]);
  let reported = false;
  let loadedSrc: string | null = $state(null);
  let sequenceStarted = false;

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
    setTimeout(() => report("timeout"), 40000);
  });

  // Tras cargar el módulo se le dispara cada acción que declaró, como haría el usuario con los botones del panel,
  // y se espera a que termine lo que ponga en marcha antes de informar.
  function onLoad(src: string) {
    loadedSrc = src;
    if (sequenceStarted || !module || moduleId === "probe") return;
    sequenceStarted = true;
    const actions = module.manifest.actions;
    actions.forEach((a, i) => {
      setTimeout(() => {
        window.dispatchEvent(new CustomEvent(MODULE_ACTION_EVENT, { detail: { module: moduleId, action: a.id } }));
      }, 1800 + i * 700);
    });
    setTimeout(() => report("settled"), 1800 + actions.length * 700 + 4500);
  }

  function onRecord(r: BridgeRecord) {
    records = [...records, r];
    // `probe.done` es la señal de fin de la sonda de permisos.
    if (r.method === "probe.done") setTimeout(() => report("done"), 300);
  }
</script>

<main>
  <h1>Prueba del módulo {moduleId}</h1>
  {#if module}
    <ModuleFrame {module} {onRecord} {onLoad} />
  {/if}
  <ul>
    {#each records as r}
      <li>{r.method}: {r.ok ? "ok" : r.error}</li>
    {/each}
  </ul>
</main>
