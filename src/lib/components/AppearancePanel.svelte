<!-- Personalización: colores de la interfaz, propios o adaptados al fondo de pantalla. -->
<script lang="ts">
  import { DEFAULT_THEME, saveTheme, theme, type Theme } from "../stores/theme";

  const FIELDS: { key: "accent" | "workspace" | "frame" | "dots"; label: string; hint: string }[] = [
    { key: "accent", label: "Acento", hint: "Pestañas, botones destacados, volumen y día de hoy" },
    { key: "workspace", label: "Workspace actual", hint: "El círculo del workspace activo en la barra" },
    { key: "frame", label: "Marco animado", hint: "El borde que rodea la ventana en uso" },
    { key: "dots", label: "Puntos del fondo", hint: "Los puntos y la luz que sigue al cursor" },
  ];
  const PRESETS: { name: string; color: string }[] = [
    { name: "Blanco", color: "#f2f2f2" },
    { name: "Cian", color: "#7fe8ff" },
    { name: "Lima", color: "#b8ff7a" },
    { name: "Ámbar", color: "#ffc062" },
    { name: "Rosa", color: "#ff8fc7" },
    { name: "Violeta", color: "#b9a2ff" },
  ];

  const custom = $derived($theme.mode === "custom");
  const set = (patch: Partial<Theme>) => saveTheme({ ...$theme, ...patch });
  /** Un color para todo a la vez. */
  const setAll = (color: string) => set({ mode: "custom", accent: color, workspace: color, frame: color, dots: color });
</script>

<section>
  <h2>Modo de color</h2>
  <div class="modes" role="radiogroup" aria-label="Modo de color">
    <button class="btn" role="radio" aria-checked={custom} class:active={custom} onclick={() => set({ mode: "custom" })}>Colores propios</button>
    <button class="btn" role="radio" aria-checked={!custom} class:active={!custom} onclick={() => set({ mode: "wallpaper" })}>Adaptar al fondo</button>
  </div>
  <p class="hint">
    {#if custom}Elige el color de cada elemento, o uno para todo a la vez.{:else}Los colores se calculan a partir del fondo de pantalla y se actualizan cuando lo cambias.{/if}
  </p>
</section>

<section class:dim={!custom}>
  <h2>Un color para todo</h2>
  <div class="swatches">
    {#each PRESETS as p (p.color)}
      <button class="btn swatch" style:--c={p.color} title={p.name} aria-label={`Todo en ${p.name}`} disabled={!custom} onclick={() => setAll(p.color)}></button>
    {/each}
  </div>
</section>

<section class:dim={!custom}>
  <h2>Cada elemento</h2>
  {#each FIELDS as f (f.key)}
    <label class="field">
      <input type="color" value={$theme[f.key]} disabled={!custom} onchange={(e) => set({ [f.key]: e.currentTarget.value })} aria-label={f.label} />
      <span><strong>{f.label}</strong><small class="hint">{f.hint}</small></span>
      <code>{$theme[f.key]}</code>
    </label>
  {/each}
</section>

<section>
  <button class="btn" onclick={() => saveTheme(DEFAULT_THEME)}>Restablecer blanco y negro</button>
</section>

<style>
  section { margin-bottom: 1.4rem; max-width: 560px; }
  h2 { margin: 0 0 8px; font: 500 11px var(--mono); text-transform: uppercase; letter-spacing: 0.16em; color: var(--muted); }
  .modes { display: flex; gap: 8px; }
  .modes .active { background: var(--accent); border-color: var(--accent); color: var(--on-accent); }
  .dim { opacity: 0.4; }
  .swatches { display: flex; gap: 10px; }
  .swatch { width: 30px; height: 30px; padding: 0; border-radius: 50%; background: var(--c); border: 2px solid rgba(255, 255, 255, 0.25); }
  .field { display: flex; align-items: center; gap: 12px; padding: 8px 0; border-bottom: 1px dotted var(--line); }
  .field span { flex: 1; display: flex; flex-direction: column; }
  .field small { font-size: 12px; }
  input[type="color"] { width: 44px; height: 30px; padding: 0; border: 1px solid var(--line); border-radius: 8px; background: transparent; cursor: pointer; }
</style>
