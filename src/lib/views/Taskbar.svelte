<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { runAction } from "../api/state";
  import { onTaskbarClosing, onTaskbarShown, taskbarApi } from "../api/shell";
  import TaskbarView from "../components/TaskbarView.svelte";
  import { createStateStore } from "../stores/wmState";

  // El id del monitor llega en la URL de la ventana: ?view=taskbar&monitor=<id>
  const monitorId = Number(new URLSearchParams(window.location.search).get("monitor") ?? 0);
  const stateStore = createStateStore();
  let openTick = $state(0);
  let closeTick = $state(0);
  let stops: (() => void)[] = [];

  onMount(() => {
    void stateStore.start();
    onTaskbarShown(() => openTick++).then((u) => stops.push(u)).catch(() => {});
    onTaskbarClosing(() => closeTick++).then((u) => stops.push(u)).catch(() => {});
  });
  onDestroy(() => {
    stateStore.stop();
    stops.forEach((u) => u());
  });
</script>

<TaskbarView {monitorId} state={$stateStore} api={taskbarApi} {openTick} {closeTick} onWorkspace={(n) => void runAction(`workspace_${n}`).catch(() => {})} />
