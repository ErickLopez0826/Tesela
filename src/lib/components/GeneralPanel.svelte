<!-- Ajustes generales: arrancar con Windows, aplicaciones que se abren con Tesela y cerrar o reiniciar la app. -->
<script lang="ts">
  import { onDestroy } from "svelte";
  import type { GeneralStatus } from "../api/general";
  import type { ShellSettings } from "../api/shell";
  import type { AppSuggestion, StartupApp } from "../api/apps";
  import { clampPage, pageCount, slicePage } from "../stores/engine";
  import AppPicker from "./AppPicker.svelte";
  import Pager from "./Pager.svelte";

  let {
    status,
    startupApps = [],
    onAutostart,
    onStartupSave = async (apps) => apps,
    onQuit,
    onRestart,
    loadApps,
    shell = null,
    onShell = async (s) => s,
  }: {
    status: GeneralStatus | null;
    /** Aplicaciones que se abren junto con Tesela, cada una con su workspace. */
    startupApps?: StartupApp[];
    onAutostart: (enabled: boolean) => Promise<void>;
    /** Guarda la lista completa y devuelve la lista ya validada. */
    onStartupSave?: (apps: StartupApp[]) => Promise<StartupApp[]>;
    /** Cierra la app por completo (se devuelven las ventanas a su sitio y desaparecen la barra y el icono de la bandeja). */
    onQuit: () => Promise<void>;
    /** Vuelve a abrir la app como estaba (con o sin gestor de ventanas y barra). */
    onRestart: (manage: boolean, bar: boolean) => Promise<void>;
    loadApps?: () => Promise<AppSuggestion[]>;
    /** Lo que Tesela reemplaza de Windows. */
    shell?: ShellSettings | null;
    onShell?: (settings: ShellSettings) => Promise<ShellSettings>;
  } = $props();

  /** El valor mostrado: el que llega y, tras guardar, el que devuelve el guardado. */
  let savedShell: ShellSettings | null = $state(null);
  const shellNow = $derived(savedShell ?? shell);
  const changeShell = (patch: Partial<ShellSettings>) =>
    shellNow ? run(async () => (savedShell = await onShell({ ...shellNow, ...patch }))) : Promise.resolve();

  const PER_PAGE = 10;
  const WORKSPACES = [1, 2, 3, 4, 5, 6, 7, 8, 9];

  let error: string | null = $state(null);
  /** Cerrar Tesela pide confirmación: el primer toque la pide y, si no se confirma, se retira sola. */
  let confirmingQuit = $state(false);
  let quitTimer: ReturnType<typeof setTimeout> | undefined;
  function askQuit() {
    confirmingQuit = true;
    clearTimeout(quitTimer);
    quitTimer = setTimeout(() => (confirmingQuit = false), 5000);
  }
  function cancelQuit() {
    clearTimeout(quitTimer);
    confirmingQuit = false;
  }
  onDestroy(() => clearTimeout(quitTimer));
  let query = $state("");
  let picked: AppSuggestion | null = $state(null);
  let newWorkspace = $state(1);
  let page = $state(0);
  /** La lista mostrada: la que llega y, en cuanto se guarda un cambio, la que devuelve el guardado. */
  let saved: StartupApp[] | null = $state(null);

  const list = $derived(saved ?? startupApps);
  const current = $derived(clampPage(page, list.length, PER_PAGE));
  // Se puede agregar una app elegida de la lista o una ruta escrita a mano.
  const canAdd = $derived(picked !== null || /[\\/]|\.exe$/i.test(query.trim()));

  async function run(op: () => Promise<unknown>) {
    error = null;
    try {
      await op();
    } catch (e) {
      error = String(e);
    }
  }

  const persist = (next: StartupApp[]) => run(async () => (saved = await onStartupSave(next)));

  function add() {
    if (!canAdd) return;
    const app: StartupApp = picked
      ? { name: picked.name, path: picked.path, exe: picked.exe, workspace: newWorkspace }
      : { name: "", path: query.trim(), exe: "", workspace: newWorkspace };
    void persist([...list, app]).then(() => {
      query = "";
      picked = null;
    });
  }

  const setWorkspace = (app: StartupApp, workspace: number) => persist(list.map((a) => (a === app ? { ...a, workspace } : a)));
  const remove = (app: StartupApp) => persist(list.filter((a) => a !== app));
</script>

{#if status}
  <section>
    <label class="check">
      <input type="checkbox" checked={status.autostart_enabled} onchange={(e) => run(() => onAutostart((e.currentTarget as HTMLInputElement).checked))} />
      Iniciar Tesela al entrar en Windows
    </label>
  </section>

  {#if shellNow}
    <section>
      <h2>Interfaz de Windows</h2>
      <label class="check">
        <input type="checkbox" checked={shellNow.replace_taskbar} onchange={(e) => changeShell({ replace_taskbar: (e.currentTarget as HTMLInputElement).checked })} />
        Reemplazar la barra de tareas de Windows por la de Tesela
      </label>
      <label class="check">
        <input type="checkbox" checked={shellNow.win_key_panel} onchange={(e) => changeShell({ win_key_panel: (e.currentTarget as HTMLInputElement).checked })} />
        La tecla Windows abre el panel de Tesela (los atajos con Windows, como Win+Shift+S, siguen igual)
      </label>
      <p class="hint">Al cerrar Tesela, la barra de tareas y la tecla Windows vuelven a ser las de siempre.</p>
    </section>
  {/if}

  <section>
    <h2>Aplicaciones de arranque</h2>
    <p class="hint">Se abren junto con Tesela y se quedan en el workspace que elijas.</p>
    {#if list.length === 0}<p class="hint">Ninguna.</p>{/if}
    <ul class="startup">
      {#each slicePage(list, current, PER_PAGE) as app (app.path)}
        <li>
          <span class="name" title={app.path}>{app.name}</span>
          <label class="ws">
            Workspace
            <select aria-label={`Workspace de ${app.name}`} value={app.workspace} onchange={(e) => setWorkspace(app, Number(e.currentTarget.value))}>
              {#each WORKSPACES as n (n)}<option value={n}>{n}</option>{/each}
            </select>
          </label>
          <button aria-label={`Quitar ${app.name}`} onclick={() => remove(app)}>Quitar</button>
        </li>
      {/each}
    </ul>
    <Pager page={current} pages={pageCount(list.length, PER_PAGE)} onPage={(p) => (page = p)} label="Páginas de aplicaciones de arranque" />
    <form
      class="add"
      onsubmit={(e) => {
        e.preventDefault();
        add();
      }}
    >
      <AppPicker
        bind:value={query}
        label="Aplicación de arranque"
        placeholder="Buscar aplicación para abrir al arrancar"
        load={loadApps}
        onPick={(app) => {
          picked = app;
          query = app.name;
        }}
      />
      <label class="ws">
        Workspace
        <select aria-label="Workspace de la nueva aplicación" bind:value={newWorkspace}>
          {#each WORKSPACES as n (n)}<option value={n}>{n}</option>{/each}
        </select>
      </label>
      <button type="submit" disabled={!canAdd}>Agregar</button>
    </form>
    <p class="hint">Si Tesela se reinicia desde aquí, las aplicaciones que ya están abiertas no se vuelven a abrir.</p>
  </section>

  <section>
    <div class="row">
      {#if confirmingQuit}
        <button class="close" onclick={() => run(onQuit)}>Sí, cerrar</button>
        <button onclick={cancelQuit}>Cancelar</button>
      {:else}
        <button class="close" onclick={askQuit}>Cerrar Tesela</button>
        <button onclick={() => run(() => onRestart(status.managing, status.bar))}>Reiniciar Tesela</button>
      {/if}
    </div>
    <p class="hint">Al cerrar, las ventanas vuelven a su sitio y desaparecen la barra y el icono de la bandeja.</p>
  </section>
{:else}
  <p class="hint">Cargando…</p>
{/if}

{#if error}<p class="error" role="alert">{error}</p>{/if}

<style>
  section { margin-bottom: 1.2rem; }
  h2 { margin: 0 0 6px; font: 500 11px var(--mono, monospace); text-transform: uppercase; letter-spacing: 0.16em; color: var(--muted, #8b8b90); }
  .check { display: flex; align-items: center; gap: 10px; font-size: 14px; cursor: pointer; }
  .row { display: flex; gap: 0.6rem; align-items: center; flex-wrap: wrap; }
  .close { border-color: #ff5a5f !important; color: #ff5a5f !important; background: transparent !important; }
  .close:hover:not(:disabled) { background: #ff5a5f !important; color: #000 !important; }
  .startup { list-style: none; margin: 6px 0 0; padding: 0; }
  .startup li { display: flex; align-items: center; gap: 10px; padding: 6px 0; border-bottom: 1px dotted var(--line, rgba(255, 255, 255, 0.14)); }
  .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 14px; }
  .ws { display: flex; align-items: center; gap: 6px; font: 11px var(--mono, monospace); text-transform: uppercase; letter-spacing: 0.06em; color: var(--muted, #8b8b90); }
  .add { display: flex; gap: 0.5rem; flex-wrap: wrap; align-items: center; margin-top: 10px; }
  .hint { opacity: 0.7; font-size: 12px; }
  .error { color: #d9534f; }
</style>
