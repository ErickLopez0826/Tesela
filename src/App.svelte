<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import "./lib/styles/nothing.css";
  import DotField from "./lib/components/DotField.svelte";
  import ModuleSelfTest from "./lib/views/ModuleSelfTest.svelte";
  import ExclusionPanel from "./lib/components/ExclusionPanel.svelte";
  import GeneralPanel from "./lib/components/GeneralPanel.svelte";
  import AppearancePanel from "./lib/components/AppearancePanel.svelte";
  import ModulesPanel from "./lib/components/ModulesPanel.svelte";
  import { generalApi, type GeneralStatus } from "./lib/api/general";
  import { modulesStore } from "./lib/stores/modules";
  import WindowList from "./lib/components/WindowList.svelte";
  import { exclusionsStore } from "./lib/stores/exclusions";
  import { windowsStore } from "./lib/stores/windows";

  let tab: "windows" | "exclusions" | "modules" | "general" | "appearance" = $state("windows");
  let general: GeneralStatus | null = $state(null);
  // Solo con `--selftest-module <id>`: la ventana principal prueba ese módulo de extremo a extremo.
  let selftest: string | null = $state(null);

  onMount(() => {
    invoke<string | null>("selftest_config").then((m) => (selftest = m)).catch(() => {});
    windowsStore.refresh();
    exclusionsStore.refresh();
    void modulesStore.start();
    generalApi.status().then((s) => (general = s)).catch(() => {});
  });
</script>

{#if selftest}
  <ModuleSelfTest moduleId={selftest} />
{:else}
<DotField />
<main>
  <nav>
    <img class="logo" src="/logo.png" alt="Tesela" width="26" height="26" />
    <button class:active={tab === "windows"} onclick={() => (tab = "windows")}>Ventanas</button>
    <button class:active={tab === "exclusions"} onclick={() => (tab = "exclusions")}>Exclusiones</button>
    <button class:active={tab === "modules"} onclick={() => (tab = "modules")}>Módulos</button>
    <button class:active={tab === "general"} onclick={() => (tab = "general")}>General</button>
    <button class:active={tab === "appearance"} onclick={() => (tab = "appearance")}>Personalización</button>
    <button onclick={() => windowsStore.refresh()} disabled={$windowsStore.loading}>Refrescar</button>
  </nav>

  {#if $windowsStore.error}
    <p>Error: {$windowsStore.error}</p>
  {/if}
  {#if $exclusionsStore.error}
    <p>Error: {$exclusionsStore.error}</p>
  {/if}

  {#if tab === "windows"}
    <h1>Ventanas visibles</h1>
    <WindowList windows={$windowsStore.windows} />
  {:else if tab === "general"}
    <h1>General</h1>
    <GeneralPanel
      status={general}
      onAutostart={async (enabled) => (general = await generalApi.setAutostart(enabled))}
      onRestart={(manage, bar) => generalApi.restart(manage, bar)}
      onCheckUpdates={generalApi.checkUpdates}
      onInstallUpdate={generalApi.installUpdate}
    />
  {:else if tab === "appearance"}
    <h1>Personalización</h1>
    <AppearancePanel />
  {:else if tab === "modules"}
    <h1>Módulos</h1>
    <ModulesPanel
      modules={$modulesStore}
      onPreview={modulesStore.preview}
      onInstall={modulesStore.install}
      onUninstall={modulesStore.uninstall}
      onSetGranted={modulesStore.setGranted}
      onSetEnabled={modulesStore.setEnabled}
      bundledDir={general?.bundled_modules_dir ?? null}
    />
  {:else}
    <h1>Exclusiones</h1>
    <ExclusionPanel
      windows={$windowsStore.windows}
      rules={$exclusionsStore.rules}
      onAdd={(exe, title) => exclusionsStore.add(exe, title)}
      onRemove={(r) => exclusionsStore.remove(r.exe, r.title_contains)}
    />
  {/if}
</main>
{/if}

<style>
  main { font-family: system-ui, sans-serif; padding: 1.4rem 1.6rem; position: relative; }
  nav { display: flex; gap: 0.5rem; margin-bottom: 1.4rem; flex-wrap: wrap; padding: 6px; width: fit-content; border: 1px solid var(--line); border-radius: 999px; background: rgba(0, 0, 0, 0.45); backdrop-filter: blur(10px); }
  nav { align-items: center; }
  .logo { border-radius: 8px; margin: 0 4px 0 2px; }
  nav button { border-color: transparent; background: transparent; }
  nav .active { background: var(--accent); border-color: var(--accent); color: var(--on-accent); box-shadow: 0 0 22px -6px rgba(255, 255, 255, 0.8); }
  nav .active:hover:not(:disabled) { background: var(--accent); }
</style>
