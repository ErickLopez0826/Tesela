<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { closePanel, runAction } from "../api/state";
  import { systemApi } from "../api/system";
  import type { ActionOutcome, SystemMode } from "../api/system";
  import PanelView from "../components/PanelView.svelte";
  import { createPollingStore } from "../stores/polling";
  import { createModulesStore } from "../stores/modules";
  import { createStateStore } from "../stores/wmState";

  const stateStore = createStateStore();
  const modules = createModulesStore();
  const overview = createPollingStore(systemApi.overview, 2000);
  const media = createPollingStore(systemApi.mediaNow, 2000);
  const volume = createPollingStore(systemApi.volumeGet, 3000);
  const radios = createPollingStore(systemApi.radioStates, 5000);
  const wallpapers = createPollingStore(systemApi.wallpapersList, 30000);

  let now = $state(new Date());
  let mode: SystemMode = $state({ allow_changes: false });
  let clock: ReturnType<typeof setInterval>;

  onMount(() => {
    void stateStore.start();
    void modules.start();
    for (const s of [overview, media, volume, radios, wallpapers]) s.start();
    clock = setInterval(() => (now = new Date()), 1000);
    systemApi.mode().then((m) => (mode = m)).catch(() => {});
  });

  onDestroy(() => {
    stateStore.stop();
    modules.stop();
    for (const s of [overview, media, volume, radios, wallpapers]) s.stop();
    clearInterval(clock);
  });

  async function after<T>(op: Promise<T>, refresh: () => Promise<void>): Promise<T> {
    try {
      return await op;
    } finally {
      void refresh();
    }
  }
</script>

<PanelView
  state={$stateStore}
  {now}
  overview={$overview}
  media={$media}
  volume={$volume}
  radios={$radios ?? []}
  wallpapers={$wallpapers}
  allowChanges={mode.allow_changes}
  modules={$modules}
  onWorkspace={(n) => void runAction(`workspace_${n}`).catch(() => {})}
  onClose={() => void closePanel()}
  onMediaCommand={(c) => void after(systemApi.mediaCommand(c), media.refresh).catch(() => {})}
  onVolume={(p) => void after(systemApi.volumeSet(p), volume.refresh).catch(() => {})}
  onMute={(m) => void after(systemApi.volumeMute(m), volume.refresh).catch(() => {})}
  onRadio={(k, on): Promise<ActionOutcome> => after(systemApi.radioSet(k, on), radios.refresh)}
  onPower={(a): Promise<ActionOutcome> => systemApi.powerRun(a)}
  onWallpaper={(n): Promise<ActionOutcome> => after(systemApi.wallpaperSet(n), wallpapers.refresh)}
/>
