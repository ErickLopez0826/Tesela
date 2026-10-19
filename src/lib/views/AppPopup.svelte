<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { onPopupClosing, onPopupOpen, popupApi, type PopupTarget } from "../api/shell";
  import AppPopupView from "../components/AppPopupView.svelte";

  let target: PopupTarget | null = $state(null);
  let openTick = $state(0);
  let closeTick = $state(0);
  let stops: (() => void)[] = [];

  onMount(() => {
    onPopupOpen((t) => {
      target = t;
      openTick++;
    }).then((u) => stops.push(u)).catch(() => {});
    onPopupClosing(() => closeTick++).then((u) => stops.push(u)).catch(() => {});
  });
  onDestroy(() => stops.forEach((u) => u()));
</script>

<AppPopupView {target} api={popupApi} {openTick} {closeTick} />
