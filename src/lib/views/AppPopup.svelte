<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { claudeApi, USAGE_CARD_KEY } from "../api/claude";
  import { onPopupClosing, onPopupOpen, popupApi, type PopupTarget } from "../api/shell";
  import AppPopupView from "../components/AppPopupView.svelte";
  import UsageCardView from "../components/UsageCardView.svelte";

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

{#if target?.exe === USAGE_CARD_KEY}
  <UsageCardView load={claudeApi.usageGet} loadActivity={claudeApi.activity} resize={popupApi.resize} {openTick} {closeTick} />
{:else}
  <AppPopupView {target} api={popupApi} {openTick} {closeTick} />
{/if}
