<!-- Detalle de la salida de audio: los dispositivos de salida (altavoces, auriculares, monitores...) con el mismo aspecto
     que las redes Wi-Fi y los dispositivos Bluetooth. La lógica (leer, elegir) vive en ConnectivitySection. -->
<script lang="ts">
  import type { ActionOutcome, AudioDevice } from "../../api/system";

  let {
    devices,
    loading,
    pending,
    outcome,
    error,
    onChoose,
    onSettings,
  }: {
    devices: AudioDevice[];
    loading: boolean;
    pending: string | null;
    outcome: ActionOutcome | null;
    error: string | null;
    onChoose: (device: AudioDevice) => void;
    onSettings: () => void;
  } = $props();
</script>

<div class="details">
  <div class="head"><span>Salida de audio</span></div>
  <ul class="list" role="radiogroup" aria-label="Dispositivo de salida">
    {#each devices as d (d.id)}
      <li class:current={d.default}>
        <button class="row" role="radio" aria-checked={d.default} disabled={pending !== null} onclick={() => onChoose(d)}>
          <span class="dot" class:on={d.default}></span>
          <span class="name">{d.name}</span>
          <small>{d.default ? "en uso" : pending === d.id ? "cambiando…" : "usar"}</small>
        </button>
      </li>
    {:else}
      <li class="hint">{loading ? "Leyendo…" : "No hay dispositivos de salida de audio."}</li>
    {/each}
  </ul>
  <button class="btn mini" onclick={onSettings}>Más ajustes de sonido</button>
  {#if outcome}<div class="notice" class:simulated={outcome.simulated}>{outcome.description}</div>{/if}
  {#if error}<div class="notice error">{error}</div>{/if}
</div>

<style>
  .details { display: flex; flex-direction: column; gap: 8px; margin-top: 10px; padding-top: 10px; border-top: 1px dotted var(--line); }
  .head { display: flex; align-items: center; justify-content: space-between; font: 11px var(--mono); text-transform: uppercase; letter-spacing: 0.1em; color: var(--muted); }
  .mini { padding: 3px 10px; font-size: 10px; align-self: flex-start; }
  /* Tres filas a la vista; con más dispositivos se desplaza. */
  .list { list-style: none; margin: 0; padding: 0 4px 0 0; display: flex; flex-direction: column; gap: 4px; max-height: 128px; overflow-y: auto; scrollbar-width: thin; scrollbar-color: rgba(255, 255, 255, 0.35) transparent; }
  .list li.hint { padding: 4px 2px; }
  .row { display: flex; align-items: center; gap: 10px; width: 100%; min-height: 38px; box-sizing: border-box; padding: 7px 10px; border-radius: 12px; text-align: left; text-transform: none; letter-spacing: 0; font: 12px var(--mono); color: var(--fg); background: rgba(255, 255, 255, 0.04); border: 1px solid transparent; cursor: pointer; }
  .row:disabled { cursor: default; }
  .current > .row { border-color: var(--accent); cursor: default; }
  .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  small { color: var(--muted); font: 10px var(--mono); }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: rgba(255, 255, 255, 0.25); }
  .dot.on { background: var(--accent); box-shadow: 0 0 8px var(--accent); }
</style>
