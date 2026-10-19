<script lang="ts">
  import Icon, { type IconName } from "../Icon.svelte";
  import { onMount } from "svelte";
  import type { ActionOutcome, AudioApi, AudioDevice, NetworkApi, RadioKindName, RadioState } from "../../api/system";
  import AudioDetails from "./AudioDetails.svelte";
  import ConnectivityDetails from "./ConnectivityDetails.svelte";
  import { signalText, wifiArcs } from "../../stores/wifi";

  let {
    radios,
    allowChanges,
    onSet,
    network = null,
    audio = null,
    onAudioChanged = () => {},
  }: {
    radios: RadioState[];
    allowChanges: boolean;
    onSet: (kind: RadioKindName, on: boolean) => Promise<ActionOutcome>;
    /// Con esto cada conexión ofrece su detalle (redes, dispositivos, zona compartida).
    network?: NetworkApi | null;
    /// Con esto aparece el botón para elegir la salida de audio.
    audio?: AudioApi | null;
    onAudioChanged?: () => void;
  } = $props();

  let open: RadioKindName | "audio" | null = $state(null);

  // Salida de audio: una ficha más, con el mismo aspecto que las demás. Muestra el dispositivo en uso.
  let outputs: AudioDevice[] = $state([]);
  let audioLoading = $state(true);
  let audioPending: string | null = $state(null);
  let audioOutcome: ActionOutcome | null = $state(null);
  let audioError: string | null = $state(null);
  const currentOutput = $derived(outputs.find((d) => d.default));

  async function loadOutputs() {
    if (!audio) return;
    try {
      outputs = await audio.outputs();
      audioError = null;
    } catch (e) {
      audioError = String(e);
    } finally {
      audioLoading = false;
    }
  }

  onMount(() => {
    if (!audio) return;
    void loadOutputs();
    // Se vuelve a leer de vez en cuando (enchufar unos auriculares cambia la lista), salvo con la ventana oculta.
    const t = setInterval(() => document.visibilityState !== "hidden" && void loadOutputs(), 4000);
    return () => clearInterval(t);
  });

  async function chooseOutput(d: AudioDevice) {
    if (!audio || d.default || audioPending) return;
    audioPending = d.id;
    audioError = null;
    try {
      audioOutcome = await audio.setOutput(d.id);
      await loadOutputs();
      onAudioChanged();
    } catch (e) {
      audioOutcome = null;
      audioError = String(e);
    } finally {
      audioPending = null;
    }
  }

  const labels: Record<RadioKindName, string> = { wifi: "Wi-Fi", bluetooth: "Bluetooth", hotspot: "Compartir red" };
  const icons: Record<RadioKindName, IconName> = { wifi: "wifi", bluetooth: "bluetooth", hotspot: "share" };

  let outcome: ActionOutcome | null = $state(null);
  let error: string | null = $state(null);

  async function toggle(r: RadioState) {
    error = null;
    try {
      outcome = await onSet(r.kind, !r.on);
    } catch (e) {
      outcome = null;
      error = String(e);
    }
  }
</script>

<section class="section">
  <h3>Conectividad {#if !allowChanges}<span class="badge safe">modo seguro: simulado</span>{/if}</h3>
  <div class="tiles">
    {#each radios as r (r.kind)}
      <div class="cell">
        <button class="tile" class:on={r.on} disabled={!r.available} aria-pressed={r.on} title={r.kind === "wifi" && r.on ? signalText(r.signal) || undefined : undefined} onclick={() => toggle(r)}>
          <!-- El icono de Wi-Fi sigue a la intensidad de la conexión: más o menos arcos según la señal. -->
          <span class="ico"><Icon name={icons[r.kind]} size={18} level={r.kind === "wifi" && r.on ? wifiArcs(r.signal) : undefined} /></span>
          <span class="txt"><strong>{labels[r.kind]}</strong><small>{r.available ? (r.on ? "sí" : "no") : "no disponible"}</small></span>
        </button>
        {#if network && r.available}
          <button class="more" class:open={open === r.kind} aria-label={`Detalles de ${labels[r.kind]}`} aria-expanded={open === r.kind} onclick={() => (open = open === r.kind ? null : r.kind)}>
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg>
          </button>
        {/if}
      </div>
    {/each}
    {#if audio}
      <div class="cell">
        <button class="tile" aria-expanded={open === "audio"} title="Cambiar la salida de audio" onclick={() => (open = open === "audio" ? null : "audio")}>
          <span class="ico"><Icon name="volume" size={18} /></span>
          <span class="txt"><strong>Audio</strong><small>{currentOutput?.name ?? (audioLoading ? "leyendo…" : "sin dispositivo")}</small></span>
        </button>
        <button class="more" class:open={open === "audio"} aria-label="Salidas de audio" aria-expanded={open === "audio"} onclick={() => (open = open === "audio" ? null : "audio")}>
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg>
        </button>
      </div>
    {/if}
  </div>
  {#if audio && open === "audio"}
    <AudioDetails devices={outputs} loading={audioLoading} pending={audioPending} outcome={audioOutcome} error={audioError} onChoose={chooseOutput} onSettings={() => audio.openSettings("sound")} />
  {/if}
  {#if network && open && open !== "audio"}
    {#key open}
      <ConnectivityDetails kind={open} api={network} radioOn={radios.find((r) => r.kind === open)?.on ?? false} />
    {/key}
  {/if}
  {#if outcome}<div class="notice" class:simulated={outcome.simulated}>{outcome.description}</div>{/if}
  {#if error}<div class="notice error">{error}</div>{/if}
</section>

<style>
  .tiles { display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 8px; }
  .cell { position: relative; }
  .cell .tile { width: 100%; box-sizing: border-box; padding-right: 36px; }
  .more { position: absolute; right: 8px; top: 50%; width: 24px; height: 24px; padding: 0; margin-top: -12px; display: grid; place-items: center; border-radius: 50%; color: inherit; background: rgba(128, 128, 128, 0.18); border: 0; cursor: pointer; transition: transform 0.4s cubic-bezier(.3, 1.6, .5, 1); }
  .cell:has(.tile.on) .more { color: var(--on-accent); }
  .more.open { transform: rotate(180deg); }
  .tile { display: flex; align-items: center; gap: 10px; padding: 10px 12px; text-align: left; text-transform: none; letter-spacing: 0; border-radius: 18px; color: var(--fg); background: rgba(255, 255, 255, 0.05); border: 1px solid var(--line); font: inherit; cursor: pointer; transition: background 0.35s, border-color 0.35s, box-shadow 0.35s, transform 0.35s cubic-bezier(.3, 1.6, .5, 1); }
  .tile:hover:not(:disabled) { transform: translateY(-1px); border-color: var(--accent); }
  .tile:disabled { opacity: 0.4; cursor: default; }
  .tile.on { background: var(--accent); border-color: var(--accent); color: var(--on-accent); box-shadow: 0 0 20px -8px rgba(255, 255, 255, 0.9); }
  .ico { display: grid; place-items: center; width: 30px; height: 30px; border-radius: 50%; background: rgba(255, 255, 255, 0.1); transition: background 0.35s; }
  .tile.on .ico { background: rgba(0, 0, 0, 0.12); }
  .txt { display: flex; flex-direction: column; line-height: 1.2; min-width: 0; }
  .txt strong { font: 600 12px var(--mono); }
  .txt small { opacity: 0.7; font: 10px var(--mono); text-transform: uppercase; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
