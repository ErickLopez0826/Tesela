<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { runAction } from "../api/state";
  import { systemApi } from "../api/system";
  import BarView from "../components/BarView.svelte";
  import { installBorderSync } from "../stores/borderSync";
  import { createPollingStore } from "../stores/polling";
  import { createModulesStore } from "../stores/modules";
  import { merida } from "../stores/weather";
  import { createStateStore, createStatsStore } from "../stores/wmState";

  // El id del monitor llega en la URL de la ventana: ?view=bar&monitor=<id>
  const monitorId = Number(new URLSearchParams(window.location.search).get("monitor") ?? 0);

  const stateStore = createStateStore();
  const modules = createModulesStore();
  const statsStore = createStatsStore();
  const volume = createPollingStore(systemApi.volumeGet, 3000);
  // Sin batería (un equipo de escritorio) llega `null` y la barra no muestra nada.
  const battery = createPollingStore(systemApi.batteryGet, 15000);
  // El consumo de la gráfica sale del resumen del sistema.
  const overview = createPollingStore(systemApi.overview, 5000);
  let now = $state(new Date());
  let clock: ReturnType<typeof setInterval>;

  onMount(() => {
    const stopBorderSync = installBorderSync();
    void stateStore.start();
    void modules.start();
    statsStore.start();
    volume.start();
    battery.start();
    overview.start();
    clock = setInterval(() => (now = new Date()), 1000);
    return stopBorderSync;
  });

  onDestroy(() => {
    stateStore.stop();
    modules.stop();
    statsStore.stop();
    volume.stop();
    battery.stop();
    overview.stop();
    clearInterval(clock);
  });
</script>

<BarView
  {monitorId}
  state={$stateStore}
  stats={$statsStore}
  volume={$volume}
  battery={$battery}
  gpu={$overview?.gpu_percent ?? null}
  temperature={$merida?.temperature ?? null}
  weatherCode={$merida?.code ?? null}
  modules={$modules}
  {now}
  onWorkspace={(n) => void runAction(`workspace_${n}`).catch(() => {})}
  onMute={(m) => void systemApi.volumeMute(m).then(() => volume.refresh()).catch(() => {})}
/>
