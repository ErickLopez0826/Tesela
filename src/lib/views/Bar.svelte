<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { openPanel, runAction } from "../api/state";
  import { systemApi } from "../api/system";
  import BarView from "../components/BarView.svelte";
  import { createPollingStore } from "../stores/polling";
  import { createStateStore, createStatsStore } from "../stores/wmState";

  // El id del monitor llega en la URL de la ventana: ?view=bar&monitor=<id>
  const monitorId = Number(new URLSearchParams(window.location.search).get("monitor") ?? 0);

  const stateStore = createStateStore();
  const statsStore = createStatsStore();
  const volume = createPollingStore(systemApi.volumeGet, 3000);
  const media = createPollingStore(systemApi.mediaNow, 3000);
  // La temperatura viene del resumen del sistema; si el equipo no la expone se omite.
  const overview = createPollingStore(systemApi.overview, 5000);
  let now = $state(new Date());
  let clock: ReturnType<typeof setInterval>;

  onMount(() => {
    void stateStore.start();
    statsStore.start();
    volume.start();
    media.start();
    overview.start();
    clock = setInterval(() => (now = new Date()), 1000);
  });

  onDestroy(() => {
    stateStore.stop();
    statsStore.stop();
    volume.stop();
    media.stop();
    overview.stop();
    clearInterval(clock);
  });
</script>

<BarView
  {monitorId}
  state={$stateStore}
  stats={$statsStore}
  volume={$volume}
  media={$media}
  temperature={$overview?.temperature_c ?? null}
  {now}
  onWorkspace={(n) => void runAction(`workspace_${n}`).catch(() => {})}
  onPanel={() => void openPanel(monitorId)}
  onMute={(m) => void systemApi.volumeMute(m).then(() => volume.refresh()).catch(() => {})}
/>
