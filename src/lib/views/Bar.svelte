<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onDestroy, onMount } from "svelte";
  import { runAction } from "../api/state";
  import { claudeApi, USAGE_CARD_KEY } from "../api/claude";
  import { systemApi } from "../api/system";
  import BarView from "../components/BarView.svelte";
  import { installBorderSync } from "../stores/borderSync";
  import { createPollingStore } from "../stores/polling";
  import { createModulesStore } from "../stores/modules";
  import { createStateStore, createStatsStore } from "../stores/wmState";

  // El id del monitor llega en la URL de la ventana: ?view=bar&monitor=<id>
  const monitorId = Number(new URLSearchParams(window.location.search).get("monitor") ?? 0);

  const stateStore = createStateStore();
  const modules = createModulesStore();
  const statsStore = createStatsStore();
  const volume = createPollingStore(systemApi.volumeGet, 3000);
  // Sin batería (un equipo de escritorio) llega `null` y la barra no muestra nada.
  const battery = createPollingStore(systemApi.batteryGet, 15000);
  // Lo que hace Claude Code, según los avisos de sus hooks (archivos locales, sin red).
  const claude = createPollingStore(claudeApi.activity, 2000);
  // El uso de la suscripción lo calcula el núcleo (cada 5 min como mucho, y solo si se activó en Ajustes): aquí solo se lee lo ya guardado.
  const claudeUsage = createPollingStore(claudeApi.usageGet, 20000);

  // La tarjeta del uso sale tras una breve pausa con el cursor encima (para que pasar de largo no la abra). Cerrarla es cosa del
  // núcleo: mira el cursor y la cierra cuando lleva un momento fuera del anillo y de la tarjeta (se puede pasar de uno a otra).
  const HOVER_MS = 140;
  let hoverTimer: ReturnType<typeof setTimeout> | undefined;
  function openUsageCard(a: { x: number; y: number; w: number; h: number }) {
    clearTimeout(hoverTimer);
    hoverTimer = setTimeout(() => {
      void invoke("popup_open", { exe: USAGE_CARD_KEY, name: "Uso de Claude", path: "", x: a.x, y: a.y, w: a.w, h: a.h, hover: true }).catch(() => {});
    }, HOVER_MS);
  }
  /** El cursor se fue antes de la pausa: no se abre. */
  function cancelUsageCard() {
    clearTimeout(hoverTimer);
  }
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
    claude.start();
    claudeUsage.start();
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
    claude.stop();
    claudeUsage.stop();
    clearTimeout(hoverTimer);
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
  claude={$claude}
  gpu={$overview?.gpu_percent ?? null}
  claudeUsage={$claudeUsage}
  onUsageHover={openUsageCard}
  onUsageLeave={cancelUsageCard}
  modules={$modules}
  {now}
  onWorkspace={(n) => void runAction(`workspace_${n}`).catch(() => {})}
  onMute={(m) => void systemApi.volumeMute(m).then(() => volume.refresh()).catch(() => {})}
/>
