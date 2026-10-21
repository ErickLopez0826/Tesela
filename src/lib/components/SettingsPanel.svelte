<!-- Ajustes dentro del panel desplegable: aplicaciones que Tesela no gestiona y ajustes generales. -->
<script lang="ts">
  import { onMount } from "svelte";
  import ClaudeSettings from "./ClaudeSettings.svelte";
  import ExclusionPanel from "./ExclusionPanel.svelte";
  import GeneralPanel from "./GeneralPanel.svelte";
  import SearchSettings from "./SearchSettings.svelte";
  import { appsApi, type StartupApp } from "../api/apps";
  import { claudeApi } from "../api/claude";
  import { generalApi, type GeneralStatus } from "../api/general";
  import { searchSettingsApi } from "../api/search";
  import { shellApi, type ShellSettings } from "../api/shell";
  import { exclusionsStore } from "../stores/exclusions";
  import { windowsStore } from "../stores/windows";

  /** Sube cada vez que el panel se abre: la lista de ventanas abiertas se vuelve a leer. */
  let { tick = 0 }: { tick?: number } = $props();

  type Section = "exclusions" | "search" | "claude" | "general";
  const SECTIONS: { id: Section; label: string }[] = [
    { id: "exclusions", label: "Aplicaciones excluidas" },
    { id: "search", label: "Buscador" },
    { id: "claude", label: "Claude" },
    { id: "general", label: "General" },
  ];

  let section: Section = $state("exclusions");
  let general: GeneralStatus | null = $state(null);
  let startupApps: StartupApp[] = $state([]);
  let shell: ShellSettings | null = $state(null);

  onMount(() => {
    void exclusionsStore.refresh();
    generalApi.status().then((s) => (general = s)).catch(() => {});
    appsApi.startupList().then((l) => (startupApps = l)).catch(() => {});
    shellApi.get().then((s) => (shell = s)).catch(() => {});
  });

  $effect(() => {
    void tick;
    void windowsStore.refresh();
  });
</script>

<div class="settings">
  <nav class="sub" aria-label="Ajustes">
    {#each SECTIONS as s (s.id)}
      <button class="btn" class:on={section === s.id} aria-pressed={section === s.id} onclick={() => (section = s.id)}>{s.label}</button>
    {/each}
  </nav>

  {#if section === "exclusions"}
    <p class="hint">Las aplicaciones de esta lista no se mueven ni se ajustan: Tesela las deja exactamente como están.</p>
    {#if $exclusionsStore.error}<p class="problem">Error: {$exclusionsStore.error}</p>{/if}
    {#if $windowsStore.error}<p class="problem">Error: {$windowsStore.error}</p>{/if}
    <ExclusionPanel
      windows={$windowsStore.windows}
      rules={$exclusionsStore.rules}
      onAdd={(exe, title) => exclusionsStore.add(exe, title)}
      onRemove={(r) => exclusionsStore.remove(r.exe, r.title_contains)}
    />
  {:else if section === "search"}
    <SearchSettings api={searchSettingsApi} />
  {:else if section === "claude"}
    <ClaudeSettings api={claudeApi} />
  {:else}
    <GeneralPanel
      status={general}
      {startupApps}
      onStartupSave={appsApi.startupSave}
      onAutostart={async (enabled) => (general = await generalApi.setAutostart(enabled))}
      {shell}
      onShell={shellApi.set}
      onQuit={generalApi.quit}
      onRestart={(manage, bar) => generalApi.restart(manage, bar)}
    />
  {/if}
</div>

<style>
  .sub { display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 12px; }
  .problem { color: #ff5a5f; font-size: 12px; }
  /* Los apartados traen sus controles sin clases: aquí se visten igual que el resto del panel. */
  .settings :global(h2) { margin: 14px 0 6px; font: 500 11px var(--mono); text-transform: uppercase; letter-spacing: 0.16em; color: var(--muted); }
  .settings :global(button:not(.btn)) {
    font: 11px var(--mono); letter-spacing: 0.08em; text-transform: uppercase; color: var(--fg);
    background: var(--panel); border: 1px solid var(--line); border-radius: 999px; padding: 6px 12px; cursor: pointer;
    transition: background 0.35s ease, border-color 0.35s ease, transform 0.35s cubic-bezier(.3, 1.6, .5, 1);
  }
  .settings :global(button:not(.btn):hover:not(:disabled)) { border-color: var(--accent); background: rgba(255, 255, 255, 0.12); transform: translateY(-1px); }
  .settings :global(button:not(.btn):active:not(:disabled)) { transform: scale(0.94); }
  .settings :global(button:not(.btn):disabled) { opacity: 0.35; cursor: default; }
  .settings :global(input:not([type="checkbox"]):not([type="file"]):not([type="range"])) {
    font: 13px var(--mono); color: var(--fg); background: rgba(0, 0, 0, 0.5); border: 1px solid var(--line);
    border-radius: 10px; padding: 7px 10px; min-width: 0;
  }
  .settings :global(select) {
    font: 12px var(--mono); color: var(--fg); background: rgba(0, 0, 0, 0.5); border: 1px solid var(--line);
    border-radius: 10px; padding: 5px 8px; cursor: pointer;
  }
  .settings :global(select option) { background: #121214; color: #f1f1f1; }
  .settings :global(input:focus) { border-color: var(--accent); outline: none; }
  .settings :global(table) { border-collapse: collapse; width: 100%; font: 12px var(--mono); }
  .settings :global(th), .settings :global(td) { border-bottom: 1px dotted var(--line) !important; text-align: left; }
  .settings :global(tbody tr:hover) { background: rgba(255, 255, 255, 0.05); }
  .settings :global(article) { background: var(--panel); border: 1px solid var(--line); border-radius: 14px; padding: 10px 12px; margin-bottom: 8px; }
  .settings :global(code) { font-family: var(--mono); font-size: 0.85em; }
  .settings :global(.error) { color: #ff5a5f; }
  .settings :global(.ok) { color: #6fdc9a; }
</style>
