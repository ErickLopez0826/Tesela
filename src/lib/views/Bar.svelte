<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { openPanel, runAction } from "../api/state";
  import BarView from "../components/BarView.svelte";
  import { createStateStore, createStatsStore } from "../stores/wmState";

  // El id del monitor llega en la URL de la ventana: ?view=bar&monitor=<id>
  const monitorId = Number(new URLSearchParams(window.location.search).get("monitor") ?? 0);

  const stateStore = createStateStore();
  const statsStore = createStatsStore();
  let now = $state(new Date());
  let clock: ReturnType<typeof setInterval>;

  onMount(() => {
    void stateStore.start();
    statsStore.start();
    clock = setInterval(() => (now = new Date()), 1000);
  });

  onDestroy(() => {
    stateStore.stop();
    statsStore.stop();
    clearInterval(clock);
  });
</script>

<BarView
  {monitorId}
  state={$stateStore}
  stats={$statsStore}
  {now}
  onWorkspace={(n) => void runAction(`workspace_${n}`).catch(() => {})}
  onPanel={() => void openPanel(monitorId)}
/>
