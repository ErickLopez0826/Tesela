<script lang="ts">
  import { merida } from "../stores/weather";
  import { refreshWallpaperTheme } from "../stores/theme";
  import { invoke } from "@tauri-apps/api/core";
  import { onDestroy, onMount } from "svelte";
  import { closePanel, onPanelClosing, onPanelShown, panelResize } from "../api/state";
  import { audioApi, notesApi, systemApi } from "../api/system";
  import { tasksApi } from "../api/tasks";
  import type { ActionOutcome, SystemMode } from "../api/system";
  import PanelView from "../components/PanelView.svelte";
  import { installBorderSync } from "../stores/borderSync";
  import { createPollingStore, isHidden, onShown } from "../stores/polling";
  import { createModulesStore } from "../stores/modules";

  const modules = createModulesStore();
  const overview = createPollingStore(systemApi.overview, 2000);
  const media = createPollingStore(systemApi.mediaNow, 2000);
  const volume = createPollingStore(systemApi.volumeGet, 3000);
  const radios = createPollingStore(systemApi.radioStates, 5000);
  const wallpapers = createPollingStore(systemApi.wallpapersList, 30000);
  const engine = createPollingStore(systemApi.engineLibrary, 20000);

  let now = $state(new Date());
  let mode: SystemMode = $state({ allow_changes: false });
  let clock: ReturnType<typeof setInterval>;
  let openTick = $state(0);
  let closeTick = $state(0);
  // La portada se pide solo cuando cambia la canción (puede pesar decenas de KB).
  let mediaArt: string | null = $state(null);
  let artKey = "";
  let unlistenShown: (() => void) | undefined;
  let unlistenClosing: (() => void) | undefined;
  let stopShown: (() => void) | undefined;

  onMount(() => {
    const stopBorderSync = installBorderSync();
    onPanelShown(() => openTick++).then((u) => (unlistenShown = u)).catch(() => {});
    onPanelClosing(() => closeTick++).then((u) => (unlistenClosing = u)).catch(() => {});
    void modules.start();
    for (const s of [overview, media, volume, radios, wallpapers, engine]) s.start();
    // El reloj del panel solo corre mientras se ve.
    clock = setInterval(() => !isHidden() && (now = new Date()), 1000);
    stopShown = onShown(() => (now = new Date()));
    systemApi.mode().then((m) => (mode = m)).catch(() => {});
    // El WebView toma el foco de teclado al hacer clic: se refresca el gancho de la tecla Windows (si no, Windows abría su búsqueda).
    const refreshKeys = () => void invoke("winkey_refresh").catch(() => {});
    window.addEventListener("pointerdown", refreshKeys, true);
    window.addEventListener("focus", refreshKeys);
    // Con el panel en uso Windows no llama a los ganchos de teclado: si la tecla Windows llega a la página, se avisa al núcleo
    // para que no se abra la búsqueda de Windows y para que cuente como toque (ver `winkey::page_key`).
    const keyKind = (e: KeyboardEvent, down: boolean) => (e.key === "Meta" ? (down ? "win_down" : "win_up") : e.key === "Unidentified" || e.keyCode === 232 ? null : "other");
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Meta" && e.repeat) return;
      const k = keyKind(e, true);
      if (k) void invoke("winkey_page_key", { kind: k }).catch(() => {});
    };
    const onKeyUp = (e: KeyboardEvent) => {
      const k = keyKind(e, false);
      if (k === "win_up") void invoke("winkey_page_key", { kind: k }).catch(() => {});
    };
    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("keyup", onKeyUp, true);
    return () => {
      window.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("keyup", onKeyUp, true);
      stopBorderSync();
      window.removeEventListener("pointerdown", refreshKeys, true);
      window.removeEventListener("focus", refreshKeys);
    };
  });

  onDestroy(() => {
    modules.stop();
    for (const s of [overview, media, volume, radios, wallpapers, engine]) s.stop();
    clearInterval(clock);
    unlistenShown?.();
    unlistenClosing?.();
    stopShown?.();
  });

  $effect(() => {
    const m = $media;
    const key = m ? `${m.app}|${m.artist}|${m.title}` : "";
    if (key === artKey) return;
    artKey = key;
    if (!m) {
      mediaArt = null;
      return;
    }
    systemApi.mediaThumbnail().then((a) => (mediaArt = key === artKey ? a : mediaArt)).catch(() => (mediaArt = null));
  });

  // WinRT tarda un momento en reflejar el nuevo estado de una radio: se vuelve a leer poco después.
  async function refreshRadios() {
    await radios.refresh();
    setTimeout(() => void radios.refresh(), 1500);
  }

  // Wallpaper Engine tarda un instante en anotar el fondo nuevo en su configuración.
  async function refreshEngine() {
    // Cuando Wallpaper Engine ya anotó el fondo nuevo, la interfaz toma su color (modo «Adaptar al fondo»).
    for (const ms of [1500, 4000]) setTimeout(() => (void engine.refresh(), refreshWallpaperTheme()), ms);
  }

  async function after<T>(op: Promise<T>, refresh: () => Promise<void>): Promise<T> {
    try {
      return await op;
    } finally {
      void refresh();
    }
  }
</script>

<PanelView
  {now}
  overview={$overview}
  media={$media}
  volume={$volume}
  radios={$radios ?? []}
  wallpapers={$wallpapers}
  allowChanges={mode.allow_changes}
  modules={$modules}
  {openTick}
  {closeTick}
  {mediaArt}
  weather={$merida}
  onResize={(h) => void panelResize(h).catch(() => {})}
  onClose={() => void closePanel()}
  onMediaCommand={(c) => void after(systemApi.mediaCommand(c), media.refresh).catch(() => {})}
  onVolume={(p) => void after(systemApi.volumeSet(p), volume.refresh).catch(() => {})}
  onMute={(m) => void after(systemApi.volumeMute(m), volume.refresh).catch(() => {})}
  onRadio={(k, on): Promise<ActionOutcome> => after(systemApi.radioSet(k, on), refreshRadios)}
  network={systemApi}
  notes={notesApi}
  tasks={tasksApi}
  engine={$engine}
  onEngineApply={(id, m) => after(systemApi.engineApply(id, m), refreshEngine)}
  onEnginePath={async (p) => {
    await systemApi.engineSetPath(p);
    await engine.refresh();
    refreshWallpaperTheme();
  }}
  audio={audioApi}
  onAudioChanged={() => void volume.refresh()}
  onWallpaper={(n): Promise<ActionOutcome> => after(systemApi.wallpaperSet(n), wallpapers.refresh)}
/>
