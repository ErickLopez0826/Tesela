<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { closePanel, runAction } from "../api/state";
  import PanelView from "../components/PanelView.svelte";
  import { createStateStore, createStatsStore } from "../stores/wmState";

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

<PanelView
  state={$stateStore}
  stats={$statsStore}
  {now}
  onWorkspace={(n) => void runAction(`workspace_${n}`).catch(() => {})}
  onClose={() => void closePanel()}
/>
