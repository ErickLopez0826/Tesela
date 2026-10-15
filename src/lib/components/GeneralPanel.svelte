<script lang="ts">
  import type { GeneralStatus, UpdateInfo } from "../api/general";

  let {
    status,
    onAutostart,
    onRestart,
    onCheckUpdates,
    onInstallUpdate,
  }: {
    status: GeneralStatus | null;
    onAutostart: (enabled: boolean) => Promise<void>;
    onRestart: (manage: boolean, bar: boolean) => Promise<void>;
    onCheckUpdates: () => Promise<UpdateInfo | null>;
    onInstallUpdate: () => Promise<void>;
  } = $props();

  let error: string | null = $state(null);
  let update: UpdateInfo | null = $state(null);
  let updateMessage: string | null = $state(null);
  let confirmManage = $state(false);

  async function run(op: () => Promise<unknown>) {
    error = null;
    try {
      await op();
    } catch (e) {
      error = String(e);
    }
  }

  async function check() {
    updateMessage = null;
    update = null;
    await run(async () => {
      update = await onCheckUpdates();
      updateMessage = update ? `Hay una versión nueva: ${update.version}` : "Ya tienes la última versión.";
    });
  }
</script>

{#if status}
  <section>
    <h2>Estado</h2>
    <p>
      Versión <strong>{status.version}</strong> ·
      Gestor de ventanas: <strong>{status.managing ? "activo" : "inactivo"}</strong> ·
      Barra: <strong>{status.bar ? "activa" : "inactiva"}</strong>
    </p>
    {#if status.managing}
      <p class="hint">Si algo sale mal, <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>Shift</kbd>+<kbd>F12</kbd> devuelve todas las ventanas a su sitio y pausa el gestor.</p>
    {/if}
    <div class="row">
      {#if !status.managing}
        {#if confirmManage}
          <span>Esto reorganizará <strong>todas</strong> tus ventanas abiertas. ¿Continuar?</span>
          <button onclick={() => run(() => onRestart(true, true))}>Sí, activar gestor y barra</button>
          <button onclick={() => (confirmManage = false)}>No</button>
        {:else}
          <button onclick={() => (confirmManage = true)}>Activar gestor de ventanas y barra…</button>
          <button onclick={() => run(() => onRestart(false, true))}>Solo la barra</button>
        {/if}
      {:else}
        <button onclick={() => run(() => onRestart(false, status.bar))}>Reiniciar sin gestor de ventanas</button>
      {/if}
    </div>
  </section>

  <section>
    <h2>Arranque con Windows</h2>
    <label>
      <input type="checkbox" checked={status.autostart_enabled} onchange={(e) => run(() => onAutostart((e.currentTarget as HTMLInputElement).checked))} />
      Iniciar la app al entrar en Windows (con gestor de ventanas y barra)
    </label>
    {#if status.autostart_command}<p class="hint"><code>{status.autostart_command}</code></p>{/if}
    <p class="hint">Si GlazeWM también arranca solo, desactívalo antes: dos gestores a la vez se pelean por las ventanas y los atajos.</p>
  </section>

  <section>
    <h2>Actualizaciones</h2>
    {#if !status.updater_compiled}
      <p class="hint">Esta compilación todavía no incluye las actualizaciones automáticas (falta la clave de firma y el servidor de versiones).</p>
    {:else}
      <div class="row">
        <button onclick={check}>Buscar actualizaciones</button>
        {#if update}<button onclick={() => run(onInstallUpdate)}>Instalar {update.version} y reiniciar</button>{/if}
      </div>
      {#if updateMessage}<p>{updateMessage}</p>{/if}
    {/if}
  </section>

  <section>
    <h2>Carpetas</h2>
    <p class="hint">Configuración, exclusiones y datos de módulos: <code>{status.config_dir}</code></p>
    {#if status.bundled_modules_dir}
      <p class="hint">Módulos incluidos para instalar desde la pestaña Módulos: <code>{status.bundled_modules_dir}</code></p>
    {/if}
  </section>
{:else}
  <p class="hint">Cargando…</p>
{/if}

{#if error}<p class="error" role="alert">{error}</p>{/if}

<style>
  section { margin-bottom: 1.4rem; }
  .row { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; }
  .hint { opacity: 0.7; }
  .error { color: #d9534f; }
  kbd { padding: 0 4px; border: 1px solid #8886; border-radius: 4px; font-size: 0.85em; }
  code { word-break: break-all; }
</style>
