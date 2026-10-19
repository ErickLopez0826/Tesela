<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { runAction } from "../api/state";
  import { backgroundApi, onTaskbarClosing, onTaskbarShown, shellApi, taskbarApi } from "../api/shell";
  import { systemApi } from "../api/system";
  import type { SystemMode } from "../api/system";
  import { createPollingStore } from "../stores/polling";
  import TaskbarView from "../components/TaskbarView.svelte";
  import { createStateStore } from "../stores/wmState";

  // El id del monitor llega en la URL de la ventana: ?view=taskbar&monitor=<id>
  const monitorId = Number(new URLSearchParams(window.location.search).get("monitor") ?? 0);
  const stateStore = createStateStore();
  let openTick = $state(0);
  let closeTick = $state(0);
  let stops: (() => void)[] = [];
  // Las aplicaciones en segundo plano se consultan cada 3 s, y solo mientras la barra se ve (la ventana oculta no sondea).
  const background = createPollingStore(backgroundApi.list, 3000);
  let showBackground = $state(true);
  let mode: SystemMode = $state({ allow_changes: false });
  const refreshSettings = () => void shellApi.get().then((s) => (showBackground = s.show_background)).catch(() => {});

  onMount(() => {
    void stateStore.start();
    background.start();
    refreshSettings();
    systemApi.mode().then((m) => (mode = m)).catch(() => {});
    onTaskbarShown(() => {
      openTick++;
      refreshSettings();
      void background.refresh();
    }).then((u) => stops.push(u)).catch(() => {});
    onTaskbarClosing(() => closeTick++).then((u) => stops.push(u)).catch(() => {});
  });
  onDestroy(() => {
    stateStore.stop();
    background.stop();
    stops.forEach((u) => u());
  });
</script>

<TaskbarView {monitorId} state={$stateStore} api={taskbarApi} {openTick} {closeTick} background={showBackground ? ($background ?? []) : []} power={{ allowChanges: mode.allow_changes, run: systemApi.powerRun }} onWorkspace={(n) => void runAction(`workspace_${n}`).catch(() => {})} />
