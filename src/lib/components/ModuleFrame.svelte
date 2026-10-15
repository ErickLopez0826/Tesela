<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { modulesApi, moduleUrl, MODULE_ACTION_EVENT, type InstalledModule } from "../api/modules";
  import { actionMessage, createBridge, eventMessage, type BridgeRecord, type CallCore } from "../modules/bridge";

  let {
    module,
    height = 160,
    width = undefined,
    callCore = modulesApi.call,
    onRecord = undefined,
    onLoad = undefined,
    src = moduleUrl(module.manifest.id),
  }: {
    module: InstalledModule;
    height?: number;
    width?: number | undefined;
    callCore?: CallCore;
    onRecord?: (r: BridgeRecord) => void;
    onLoad?: (src: string) => void;
    src?: string;
  } = $props();

  let frame: HTMLIFrameElement | undefined = $state();
  const id = module.manifest.id;
  const handle = createBridge(id, callCore, onRecord);
  let unlistenEvents: (() => void) | null = null;

  function post(message: unknown) {
    frame?.contentWindow?.postMessage(message, "*");
  }

  // Solo se atiende lo que envía ESTE iframe; el id del módulo lo pone el anfitrión.
  function onMessage(ev: MessageEvent) {
    if (!frame || ev.source !== frame.contentWindow) return;
    void handle(ev.data).then((reply) => reply && post(reply));
  }

  function onAction(ev: Event) {
    const d = (ev as CustomEvent<{ module: string; action: string }>).detail;
    if (d?.module === id) post(actionMessage(d.action));
  }

  onMount(() => {
    window.addEventListener("message", onMessage);
    window.addEventListener(MODULE_ACTION_EVENT, onAction);
    modulesApi
      .onEvent((e) => e.module === id && post(eventMessage(e.event, e.payload)))
      .then((un) => (unlistenEvents = un))
      .catch(() => {});
  });

  onDestroy(() => {
    window.removeEventListener("message", onMessage);
    window.removeEventListener(MODULE_ACTION_EVENT, onAction);
    unlistenEvents?.();
  });
</script>

<!-- sandbox sin allow-same-origin: origen opaco, sin acceso a Tauri ni al resto de la app. -->
<iframe
  bind:this={frame}
  {src}
  onload={() => onLoad?.(src)}
  sandbox="allow-scripts"
  title={module.manifest.name}
  style:height={`${height}px`}
  style:width={width ? `${width}px` : "100%"}
></iframe>

<style>
  iframe { border: 0; display: block; background: transparent; color-scheme: dark; }
</style>
