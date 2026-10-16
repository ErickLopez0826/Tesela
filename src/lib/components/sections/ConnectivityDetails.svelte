<!-- Detalle de una conexión: redes Wi-Fi para conectarse, dispositivos Bluetooth o la zona compartida. -->
<script lang="ts">
  import { onMount } from "svelte";
  import type { ActionOutcome, BluetoothDevice, HotspotInfo, NetworkApi, RadioKindName, WifiNetwork } from "../../api/system";

  let { kind, api, radioOn }: { kind: RadioKindName; api: NetworkApi; radioOn: boolean } = $props();

  let busy = $state(false);
  let error: string | null = $state(null);
  let outcome: ActionOutcome | null = $state(null);

  let networks: WifiNetwork[] = $state([]);
  let picked: string | null = $state(null);
  let password = $state("");

  let devices: BluetoothDevice[] = $state([]);

  let hotspot: HotspotInfo | null = $state(null);
  let hotspotLoaded = $state(false);
  let ssid = $state("");
  let passphrase = $state("");
  let showPass = $state(false);

  async function run<T>(op: () => Promise<T>): Promise<T | undefined> {
    busy = true;
    error = null;
    try {
      return await op();
    } catch (e) {
      error = String(e);
      return undefined;
    } finally {
      busy = false;
    }
  }

  async function load() {
    outcome = null;
    if (kind === "wifi") {
      const list = await run(api.wifiNetworks);
      if (list) networks = list;
    } else if (kind === "bluetooth") {
      const list = await run(api.bluetoothDevices);
      if (list) devices = list;
    } else {
      const info = await run(api.hotspotInfo);
      if (info !== undefined) {
        hotspot = info;
        hotspotLoaded = true;
        ssid = info?.ssid ?? "";
        passphrase = info?.passphrase ?? "";
      }
    }
  }

  onMount(() => void load());

  async function connect(n: WifiNetwork) {
    const out = await run(() => api.wifiConnect(n.ssid, password || undefined));
    if (out) {
      outcome = out;
      picked = null;
      password = "";
      await load();
    }
  }

  async function disconnect() {
    const out = await run(api.wifiDisconnect);
    if (out) {
      outcome = out;
      await load();
    }
  }

  async function saveHotspot() {
    const out = await run(() => api.hotspotConfigure(ssid, passphrase));
    if (out) {
      outcome = out;
      await load();
    }
  }
</script>

<div class="details">
  {#if kind === "wifi"}
    <div class="head">
      <span>Redes disponibles</span>
      <button class="btn mini" onclick={load} disabled={busy}>{busy ? "Buscando…" : "Buscar de nuevo"}</button>
    </div>
    {#if !radioOn}<div class="hint">El Wi-Fi está apagado: enciéndelo para ver las redes.</div>{/if}
    <ul class="list">
      {#each networks as n (n.ssid)}
        <li class:current={n.connected}>
          <button class="row" onclick={() => (picked = picked === n.ssid ? null : n.ssid)} aria-expanded={picked === n.ssid}>
            <span class="bars" aria-label={`Señal ${n.signal}%`}>{#each [25, 50, 75, 100] as step (step)}<i class:on={n.signal >= step} style:height={`${4 + step / 12}px`}></i>{/each}</span>
            <span class="name">{n.ssid}</span>
            {#if n.secured}<small>protegida</small>{/if}
            {#if n.connected}<small class="badge safe">conectada</small>{/if}
          </button>
          {#if picked === n.ssid}
            <div class="action">
              {#if n.connected}
                <button class="btn" onclick={disconnect} disabled={busy}>Desconectar</button>
              {:else}
                {#if n.secured}
                  <input type="password" placeholder="Contraseña (vacía si ya la guardaste)" bind:value={password} aria-label={`Contraseña de ${n.ssid}`} onkeydown={(e) => e.key === "Enter" && connect(n)} />
                {/if}
                <button class="btn primary" onclick={() => connect(n)} disabled={busy}>Conectar</button>
              {/if}
            </div>
          {/if}
        </li>
      {:else}
        {#if !busy}<li class="hint">No se ven redes Wi-Fi.</li>{/if}
      {/each}
    </ul>
    <button class="btn mini" onclick={() => api.openSettings("wifi")}>Más ajustes de Wi-Fi</button>
  {:else if kind === "bluetooth"}
    <div class="head">
      <span>Dispositivos emparejados</span>
      <button class="btn mini" onclick={load} disabled={busy}>{busy ? "Leyendo…" : "Actualizar"}</button>
    </div>
    {#if !radioOn}<div class="hint">El Bluetooth está apagado: enciéndelo para ver qué hay conectado.</div>{/if}
    <ul class="list">
      {#each devices as d (d.id)}
        <li class:current={d.connected}>
          <div class="row static"><span class="dot" class:on={d.connected}></span><span class="name">{d.name}</span><small>{d.connected ? "conectado" : "emparejado"}</small></div>
        </li>
      {:else}
        {#if !busy}<li class="hint">No hay dispositivos emparejados.</li>{/if}
      {/each}
    </ul>
    <div class="hint">Windows no deja a las apps conectar ni emparejar dispositivos: eso se hace en sus ajustes.</div>
    <button class="btn mini" onclick={() => api.openSettings("bluetooth")}>Abrir ajustes de Bluetooth</button>
  {:else}
    {#if hotspotLoaded && !hotspot}
      <div class="hint">Este equipo no puede compartir su conexión ahora mismo.</div>
    {:else if hotspot}
      <div class="head"><span>Red compartida</span><small>{hotspot.clients.length} de {hotspot.max_clients} conectados</small></div>
      <div class="form">
        <label>Nombre<input bind:value={ssid} maxlength="32" aria-label="Nombre de la red compartida" /></label>
        <label>Contraseña
          <span class="pass"><input type={showPass ? "text" : "password"} bind:value={passphrase} aria-label="Contraseña de la red compartida" /><button class="btn mini" onclick={() => (showPass = !showPass)}>{showPass ? "Ocultar" : "Ver"}</button></span>
        </label>
        <button class="btn primary" onclick={saveHotspot} disabled={busy}>Guardar</button>
      </div>
      <ul class="list">
        {#each hotspot.clients as c (c.mac || c.name)}
          <li><div class="row static"><span class="dot on"></span><span class="name">{c.name || "Dispositivo"}</span><small>{c.mac}</small></div></li>
        {:else}
          <li class="hint">Nadie está conectado.</li>
        {/each}
      </ul>
    {:else if busy}
      <div class="hint">Leyendo…</div>
    {/if}
    <button class="btn mini" onclick={() => api.openSettings("hotspot")}>Abrir ajustes de zona móvil</button>
  {/if}

  {#if outcome}<div class="notice" class:simulated={outcome.simulated}>{outcome.description}</div>{/if}
  {#if error}<div class="notice error">{error}</div>{/if}
</div>

<style>
  .details { display: flex; flex-direction: column; gap: 8px; margin-top: 10px; padding-top: 10px; border-top: 1px dotted var(--line); }
  .head { display: flex; align-items: center; justify-content: space-between; font: 11px var(--mono); text-transform: uppercase; letter-spacing: 0.1em; color: var(--muted); }
  .mini { padding: 3px 10px; font-size: 10px; align-self: flex-start; }
  /* Tres filas a la vista (la lista crece hasta ahí); con más redes o aparatos se desplaza. */
  .list { list-style: none; margin: 0; padding: 0 4px 0 0; display: flex; flex-direction: column; gap: 4px; max-height: 128px; overflow-y: auto; scrollbar-width: thin; scrollbar-color: rgba(255, 255, 255, 0.35) transparent; }
  .list li.hint { padding: 4px 2px; }
  .row { display: flex; align-items: center; gap: 10px; width: 100%; min-height: 38px; box-sizing: border-box; padding: 7px 10px; border-radius: 12px; text-align: left; text-transform: none; letter-spacing: 0; font: 12px var(--mono); color: var(--fg); background: rgba(255, 255, 255, 0.04); border: 1px solid transparent; }
  .row.static { cursor: default; }
  .current > .row { border-color: var(--accent); }
  .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  small { color: var(--muted); font: 10px var(--mono); }
  .bars { display: inline-flex; align-items: flex-end; gap: 2px; height: 14px; }
  .bars i { width: 3px; border-radius: 1px; background: rgba(255, 255, 255, 0.2); }
  .bars i.on { background: var(--accent); }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: rgba(255, 255, 255, 0.25); }
  .dot.on { background: var(--accent); box-shadow: 0 0 8px var(--accent); }
  .action { display: flex; gap: 6px; padding: 6px 0 2px; }
  .action input { flex: 1; min-width: 0; }
  .form { display: flex; flex-direction: column; gap: 6px; }
  .form label { display: flex; flex-direction: column; gap: 3px; font: 10px var(--mono); text-transform: uppercase; letter-spacing: 0.1em; color: var(--muted); }
  .pass { display: flex; gap: 6px; }
  .pass input { flex: 1; min-width: 0; }
  input { font: 12px var(--mono); color: var(--fg); background: rgba(0, 0, 0, 0.5); border: 1px solid var(--line); border-radius: 10px; padding: 6px 10px; }
  input:focus { outline: none; border-color: var(--accent); }
</style>
