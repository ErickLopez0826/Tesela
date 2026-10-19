<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onDestroy, onMount } from "svelte";
  import { onSearchClosing, onSearchShown, searchApi } from "../api/search";
  import SearchView from "../components/SearchView.svelte";

  let openTick = $state(0);
  let closeTick = $state(0);
  let unlistenShown: (() => void) | undefined;
  let unlistenClosing: (() => void) | undefined;

  onMount(() => {
    onSearchShown(() => openTick++).then((u) => (unlistenShown = u)).catch(() => {});
    onSearchClosing(() => closeTick++).then((u) => (unlistenClosing = u)).catch(() => {});
    // Aquí sí se escribe, así que el WebView toma el foco de teclado: con él Windows deja de llamar a los ganchos de teclado y
    // la tecla Windows abriría su búsqueda. Mismo respaldo que el panel: se refresca el gancho y, si la tecla llega a la
    // página, se avisa al núcleo para que no cuente como de Windows (ver `winkey::page_key`).
    const refreshKeys = () => void invoke("winkey_refresh").catch(() => {});
    window.addEventListener("pointerdown", refreshKeys, true);
    window.addEventListener("focus", refreshKeys);
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
      window.removeEventListener("pointerdown", refreshKeys, true);
      window.removeEventListener("focus", refreshKeys);
    };
  });

  onDestroy(() => {
    unlistenShown?.();
    unlistenClosing?.();
  });
</script>

<SearchView api={searchApi} {openTick} {closeTick} />
