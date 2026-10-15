<script lang="ts">
  import type { InstallPreview, InstalledModule, Permission } from "../api/modules";

  let {
    modules,
    onPreview,
    onInstall,
    onUninstall,
    onSetGranted,
    onSetEnabled,
  }: {
    modules: InstalledModule[];
    onPreview: (path: string) => Promise<InstallPreview>;
    onInstall: (path: string, granted: Permission[]) => Promise<void>;
    onUninstall: (id: string) => Promise<void>;
    onSetGranted: (id: string, granted: Permission[]) => Promise<void>;
    onSetEnabled: (id: string, enabled: boolean) => Promise<void>;
  } = $props();

  let path = $state("");
  let preview: InstallPreview | null = $state(null);
  let accepted: Permission[] = $state([]);
  let error: string | null = $state(null);
  let message: string | null = $state(null);
  let confirmRemove: string | null = $state(null);

  async function review() {
    error = null;
    message = null;
    preview = null;
    try {
      preview = await onPreview(path.trim());
      // Por defecto nada concedido: el usuario marca lo que acepta (§10.3).
      accepted = [];
    } catch (e) {
      error = String(e);
    }
  }

  function toggleAccepted(p: Permission) {
    accepted = accepted.includes(p) ? accepted.filter((x) => x !== p) : [...accepted, p];
  }

  async function install() {
    if (!preview) return;
    error = null;
    try {
      await onInstall(path.trim(), accepted);
      message = `«${preview.manifest.name}» instalado.`;
      preview = null;
      path = "";
    } catch (e) {
      error = String(e);
    }
  }

  async function remove(id: string) {
    error = null;
    try {
      await onUninstall(id);
      confirmRemove = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function toggleGranted(m: InstalledModule, p: Permission) {
    const next = m.granted.includes(p) ? m.granted.filter((x) => x !== p) : [...m.granted, p];
    try {
      await onSetGranted(m.manifest.id, next);
    } catch (e) {
      error = String(e);
    }
  }

  const permissionLabel: Record<Permission, string> = {
    windows: "Ventanas y workspaces",
    power: "Energía",
    network: "Red",
    files: "Archivos propios",
    notifications: "Avisos",
    system: "Estadísticas del sistema",
  };
</script>

<section>
  <h2>Instalar un módulo</h2>
  <p class="hint">Indica la carpeta del módulo (la que contiene <code>module.json</code>). Antes de instalar verás los permisos que pide.</p>
  <form onsubmit={(e) => { e.preventDefault(); void review(); }}>
    <input placeholder="C:\ruta\al\modulo" bind:value={path} aria-label="Carpeta del módulo" />
    <button type="submit" disabled={!path.trim()}>Revisar</button>
  </form>

  {#if preview}
    <div class="preview" role="region" aria-label="Revisión del módulo">
      <h3>{preview.manifest.name} <small>v{preview.manifest.version} · {preview.manifest.author}</small></h3>
      <p>{preview.manifest.description}</p>
      {#if preview.installed_version}<p class="hint">Ya está instalado (v{preview.installed_version}); esto lo actualizará.</p>{/if}
      <h4>Permisos que pide</h4>
      {#if preview.permissions.length === 0}
        <p class="hint">Ninguno.</p>
      {/if}
      <ul class="perms">
        {#each preview.permissions as p (p.permission)}
          <li>
            <label>
              <input type="checkbox" checked={accepted.includes(p.permission)} onchange={() => toggleAccepted(p.permission)} />
              <strong>{permissionLabel[p.permission]}</strong> — {p.description}
            </label>
          </li>
        {/each}
      </ul>
      {#if preview.network_hosts.length > 0}
        <p class="hint">Servidores a los que podrá conectarse: {preview.network_hosts.join(", ")}</p>
      {/if}
      {#if preview.manifest.actions.length > 0}
        <p class="hint">Acciones que aporta: {preview.manifest.actions.map((a) => a.title).join(", ")}</p>
      {/if}
      {#if preview.manifest.shortcuts.length > 0}
        <p class="hint">Atajos sugeridos (no se aplican solos): {preview.manifest.shortcuts.map((s) => s.keys.join(" / ")).join(", ")}</p>
      {/if}
      <button onclick={install}>Instalar con {accepted.length} {accepted.length === 1 ? "permiso" : "permisos"}</button>
      <button onclick={() => (preview = null)}>Cancelar</button>
    </div>
  {/if}

  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if message}<p class="ok">{message}</p>{/if}
</section>

<section>
  <h2>Módulos instalados ({modules.length})</h2>
  {#if modules.length === 0}<p class="hint">Ninguno todavía.</p>{/if}
  {#each modules as m (m.manifest.id)}
    <article class:off={!m.enabled}>
      <header>
        <strong>{m.manifest.name}</strong> <small>v{m.manifest.version} · {m.manifest.author}</small>
        {#if !m.enabled}<span class="tag">desactivado</span>{/if}
      </header>
      <p class="hint">{m.manifest.description}</p>
      <div class="granted">
        {#each m.manifest.permissions as p (p)}
          <label><input type="checkbox" checked={m.granted.includes(p)} onchange={() => toggleGranted(m, p)} /> {permissionLabel[p]}</label>
        {/each}
        {#if m.manifest.permissions.length === 0}<span class="hint">Sin permisos.</span>{/if}
      </div>
      <div class="row">
        <button onclick={() => onSetEnabled(m.manifest.id, !m.enabled)}>{m.enabled ? "Desactivar" : "Activar"}</button>
        {#if confirmRemove === m.manifest.id}
          <span>¿Desinstalar y borrar sus datos?</span>
          <button onclick={() => remove(m.manifest.id)}>Sí, desinstalar</button>
          <button onclick={() => (confirmRemove = null)}>No</button>
        {:else}
          <button onclick={() => (confirmRemove = m.manifest.id)}>Desinstalar</button>
        {/if}
      </div>
    </article>
  {/each}
</section>

<style>
  section { margin-bottom: 1.4rem; }
  form { display: flex; gap: 0.5rem; }
  form input { flex: 1; padding: 4px 6px; }
  .preview, article { border: 1px solid #8884; border-radius: 8px; padding: 0.7rem; margin: 0.6rem 0; }
  article.off { opacity: 0.6; }
  .perms { list-style: none; padding: 0; }
  .perms li { padding: 3px 0; }
  .granted { display: flex; flex-wrap: wrap; gap: 0.8rem; margin: 0.4rem 0; }
  .row { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; }
  .hint { opacity: 0.7; }
  .error { color: #d9534f; }
  .ok { color: #3c9a5f; }
  .tag { margin-left: 0.5rem; padding: 1px 6px; border-radius: 6px; background: #8884; font-size: 0.8em; }
</style>
