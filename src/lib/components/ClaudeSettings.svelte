<!-- Claude en Tesela: instalar o quitar los hooks (estado de Claude Code en la barra) y activar el uso de la suscripción. -->
<script lang="ts">
  import { onMount } from "svelte";
  import type { ClaudeApi, HooksStatus, UsageView } from "../api/claude";
  import { describeUsage, formatAge } from "../stores/claude";
  import { errorText } from "../stores/search";

  let { api }: { api: ClaudeApi } = $props();

  let status = $state<HooksStatus | null>(null);
  let confirming: "install" | "uninstall" | null = $state(null);
  let busy = $state(false);
  let error: string | null = $state(null);

  let usage = $state<UsageView | null>(null);
  let usageBusy = $state(false);
  let usageError: string | null = $state(null);
  let now = $state(Math.floor(Date.now() / 1000));

  onMount(() => {
    api.hooksStatus().then((s) => (status = s)).catch((e) => (error = errorText(e)));
    api.usageGet().then((u) => (usage = u)).catch((e) => (usageError = errorText(e)));
    const timer = setInterval(() => (now = Math.floor(Date.now() / 1000)), 30_000);
    return () => clearInterval(timer);
  });

  async function toggleUsage(enabled: boolean) {
    usageBusy = true;
    usageError = null;
    try {
      usage = await api.usageSetEnabled(enabled);
    } catch (e) {
      usageError = errorText(e);
    } finally {
      usageBusy = false;
    }
  }

  async function refreshUsage() {
    usageBusy = true;
    usageError = null;
    try {
      usage = await api.usageRefresh();
    } catch (e) {
      usageError = errorText(e);
    } finally {
      usageBusy = false;
    }
  }

  async function run(action: "install" | "uninstall") {
    busy = true;
    error = null;
    try {
      status = await (action === "install" ? api.hooksInstall() : api.hooksUninstall());
      confirming = null;
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  const stateText = $derived(!status ? "Comprobando…" : status.installed ? "Instalado" : status.partial ? "Instalado a medias" : "No instalado");
</script>

<section class="claude-settings" aria-label="Claude Code">
  <h2>Estado de Claude Code</h2>

  <article>
    <p class="state">
      <span>Estado:</span> <strong class:ok={status?.installed}>{stateText}</strong>
    </p>
    {#if status && !status.settings_ok}
      <p class="error" role="alert">El archivo <code>{status.settings_path}</code> no es un JSON válido: Tesela no lo toca. Arréglalo y vuelve a intentarlo.</p>
    {/if}

    {#if confirming === "install" && status}
      <p>Se modifica <code>{status.settings_path}</code> (con copia de seguridad).</p>
      <div class="buttons">
        <button disabled={busy} onclick={() => run("install")}>Confirmar instalación</button>
        <button disabled={busy} onclick={() => (confirming = null)}>Cancelar</button>
      </div>
    {:else if confirming === "uninstall" && status}
      <p>Se quitan los hooks de Tesela de <code>{status.settings_path}</code>.</p>
      <div class="buttons">
        <button disabled={busy} onclick={() => run("uninstall")}>Confirmar</button>
        <button disabled={busy} onclick={() => (confirming = null)}>Cancelar</button>
      </div>
    {:else if status}
      <div class="buttons">
        {#if !status.installed}
          <button disabled={busy || !status.settings_ok} onclick={() => (confirming = "install")}>{status.partial ? "Reparar" : "Instalar"}</button>
        {/if}
        {#if status.installed || status.partial}
          <button disabled={busy || !status.settings_ok} onclick={() => (confirming = "uninstall")}>Quitar</button>
        {/if}
      </div>
    {/if}
    {#if error}<p class="error" role="alert">{error}</p>{/if}
  </article>
</section>

<section class="claude-settings usage" aria-label="Uso de la suscripción">
  <h2>Uso de tu suscripción</h2>
  <article>
    <label class="switch">
      <input type="checkbox" checked={usage?.enabled ?? false} disabled={usageBusy || usage === null} onchange={(e) => toggleUsage(e.currentTarget.checked)} />
      Mostrar el uso de mi suscripción en la barra superior
    </label>

    {#if usage?.enabled}
      {#if usage.status === "waiting" && !usage.problem}
        <p class="state">Leyendo…</p>
      {/if}
      {#if usage.windows.length > 0}
        <pre class="detail">{describeUsage(usage, now)}</pre>
      {/if}
      {#if usage.status === "ok" && usage.fetched_at !== null}
        <p class="state">Última lectura {formatAge(usage.fetched_at, now)}.</p>
      {/if}
      {#if usage.problem && usage.message}
        <p class={usage.status === "stale" ? "state" : "error"} role={usage.status === "stale" ? undefined : "alert"}>{usage.message}</p>
      {/if}
      <div class="buttons">
        <button disabled={usageBusy} onclick={refreshUsage}>Actualizar ahora</button>
      </div>
    {/if}
    {#if usageError}<p class="error" role="alert">{usageError}</p>{/if}
  </article>
</section>

<style>
  .state { margin: 0 0 8px; font: 12px var(--mono); }
  .state span { color: var(--muted); }
  .buttons { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 8px; }
  p { margin: 6px 0; font-size: 12px; line-height: 1.45; }
  code { word-break: break-all; }
  .usage { margin-top: 14px; }
  .switch { display: flex; align-items: center; gap: 8px; margin-top: 8px; font-size: 12px; cursor: pointer; }
  .detail { margin: 8px 0 0; white-space: pre-wrap; font: 11px/1.5 var(--mono); color: var(--muted); }
</style>
