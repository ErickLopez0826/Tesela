<!-- Deslizador con barra y círculo. La barra se llena hasta el círculo y, en el máximo, queda llena por completo: el círculo
     recorre la pista de borde a borde sin salirse. El control nativo va encima, invisible, para arrastrar y para el teclado. -->
<script lang="ts">
  let {
    value,
    min = 0,
    max = 100,
    label,
    disabled = false,
    oninput = () => {},
    onchange = () => {},
  }: {
    value: number;
    min?: number;
    max?: number;
    label: string;
    disabled?: boolean;
    oninput?: (value: number) => void;
    onchange?: (value: number) => void;
  } = $props();

  const percent = $derived(max > min ? Math.max(0, Math.min(100, ((value - min) / (max - min)) * 100)) : 0);
</script>

<div class="slider" class:disabled style:--p={`${percent}%`} style:--pn={percent}>
  <div class="track"><div class="fill"></div></div>
  <i class="thumb"></i>
  <input
    type="range"
    {min}
    {max}
    step="1"
    {value}
    {disabled}
    aria-label={label}
    oninput={(e) => oninput(Number(e.currentTarget.value))}
    onchange={(e) => onchange(Number(e.currentTarget.value))}
  />
</div>

<style>
  .slider {
    --thumb: 20px;
    /* Centro del círculo: va de medio círculo a medio círculo menos del ancho, así en 0 % y en 100 % queda dentro. */
    --x: calc(var(--p) - var(--thumb) * var(--pn) / 100 + var(--thumb) / 2);
    position: relative; flex: 1; min-width: 90px; height: var(--thumb);
  }
  .track {
    position: absolute; left: 0; right: 0; top: 50%; height: 8px; margin-top: -4px; overflow: hidden; border-radius: 999px;
    background: rgba(255, 255, 255, 0.16); box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.2);
  }
  .fill {
    position: absolute; inset: 0 auto 0 0; width: var(--x); border-radius: 999px;
    background: var(--accent); box-shadow: 0 0 12px -2px var(--accent);
    transition: width 0.25s cubic-bezier(.22, 1, .36, 1);
  }
  .thumb {
    position: absolute; top: 50%; left: calc(var(--x) - var(--thumb) / 2); width: var(--thumb); height: var(--thumb); margin-top: calc(var(--thumb) / -2);
    box-sizing: border-box; border-radius: 50%; pointer-events: none;
    background: var(--accent); border: 2px solid rgba(10, 10, 11, 0.55);
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.5), 0 0 12px -2px var(--accent);
    transition: left 0.25s cubic-bezier(.22, 1, .36, 1), transform 0.25s cubic-bezier(.3, 1.8, .5, 1), box-shadow 0.25s;
  }
  .slider:hover .thumb { transform: scale(1.12); }
  /* Al arrastrar el círculo sigue al cursor sin retraso y se agranda un poco con un halo. */
  .slider:active .fill { transition: none; }
  .slider:active .thumb { transition: transform 0.25s cubic-bezier(.3, 1.8, .5, 1), box-shadow 0.25s; transform: scale(1.2); box-shadow: 0 2px 6px rgba(0, 0, 0, 0.5), 0 0 0 5px color-mix(in srgb, var(--accent) 30%, transparent); }
  input { position: absolute; inset: 0; width: 100%; height: 100%; margin: 0; opacity: 0; cursor: pointer; }
  .disabled { opacity: 0.4; }
  .disabled input { cursor: default; }
  @media (prefers-reduced-motion: reduce) { .fill, .thumb { transition: none; } }
</style>
