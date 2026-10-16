<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { runAction } from "../api/state";
  import { systemApi } from "../api/system";
  import BarView from "../components/BarView.svelte";
  import { createPollingStore } from "../stores/polling";
  import { createModulesStore } from "../stores/modules";
  import { meridaTemperature } from "../stores/weather";
  import { createStateStore, createStatsStore } from "../stores/wmState";

  // El id del monitor llega en la URL de la ventana: ?view=bar&monitor=<id>
  const monitorId = Number(new URLSearchParams(window.location.search).get("monitor") ?? 0);

  const stateStore = createStateStore();
  const modules = createModulesStore();
  const statsStore = createStatsStore();
  const volume = createPollingStore(systemApi.volumeGet, 3000);
  // El consumo de la gráfica sale del resumen del sistema.
  const overview = createPollingStore(systemApi.overview, 5000);
  let now = $state(new Date());
  let clock: ReturnType<typeof setInterval>;

  onMount(() => {
    void stateStore.start();
    void modules.start();
    statsStore.start();
    volume.start();
    overview.start();
    clock = setInterval(() => (now = new Date()), 1000);
  });

  onDestroy(() => {
    stateStore.stop();
    modules.stop();
    statsStore.stop();
    volume.stop();
    overview.stop();
    clearInterval(clock);
  });
</script>

<BarView
  {monitorId}
  state={$stateStore}
  stats={$statsStore}
  volume={$volume}
  gpu={$overview?.gpu_percent ?? null}
  temperature={$meridaTemperature}
  modules={$modules}
  {now}
  onWorkspace={(n) => void runAction(`workspace_${n}`).catch(() => {})}
  onMute={(m) => void systemApi.volumeMute(m).then(() => volume.refresh()).catch(() => {})}
/>
