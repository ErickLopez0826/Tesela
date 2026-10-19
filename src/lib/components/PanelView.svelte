<script lang="ts">
  import "../styles/panel.css";
  import type { TasksApi } from "../api/tasks";
  import type {
    ActionOutcome,
    MediaCommandName,
    MediaInfo,
    RadioKindName,
    RadioState,
    AudioApi,
    EngineLibrary,
    NotesApi,
    NetworkApi,
    SystemOverview,
    VolumeState,
    WallpaperList,
  } from "../api/system";
  import { formatClock } from "../stores/wmState";
  import { weekDays, weekRange } from "../stores/week";
  import { describeWeather, type Weather } from "../stores/weather";
  import type { InstalledModule } from "../api/modules";
  import DotField from "./DotField.svelte";
  import GlassBackdrop from "./GlassBackdrop.svelte";
  import { effectiveLuma, toneFor } from "../stores/tone";
  import { theme } from "../stores/theme";
  import WeatherIcon from "./WeatherIcon.svelte";
  import ModuleSections from "./ModuleSections.svelte";
  import AppearancePanel from "./AppearancePanel.svelte";
  import SettingsPanel from "./SettingsPanel.svelte";
  import ConnectivitySection from "./sections/ConnectivitySection.svelte";
  import EngineWallpapers from "./sections/EngineWallpapers.svelte";
  import NotesSection from "./sections/NotesSection.svelte";
  import MediaSection from "./sections/MediaSection.svelte";
  import StatsSection from "./sections/StatsSection.svelte";
  import TasksSection from "./sections/TasksSection.svelte";
  import WallpaperSection from "./sections/WallpaperSection.svelte";

  let {
    now,
    overview,
    media,
    volume,
    radios,
    wallpapers,
    allowChanges,
    modules = [],
    mediaArt = null,
    weather = null,
    network = null,
    audio = null,
    notes = null,
    tasks = null,
    engine = null,
    onEngineApply = async () => ({ simulated: true, description: "" }),
    onAudioChanged = () => {},
    openTick = 0,
    closeTick = 0,
    onResize = () => {},
    onClose,
    onMediaCommand,
    onVolume,
    onMute,
    onRadio,
    onWallpaper,
  }: {
    now: Date;
    overview: SystemOverview | null;
    media: MediaInfo | null;
    volume: VolumeState | null;
    radios: RadioState[];
    wallpapers: WallpaperList | null;
    allowChanges: boolean;
    modules?: InstalledModule[];
    /// Portada de lo que suena (data URL), si la hay.
    mediaArt?: string | null;
    /// Tiempo actual en Mérida, si hay conexión.
    weather?: Weather | null;
    /// Redes, dispositivos Bluetooth y zona compartida (detalles de la pestaña Controles).
    network?: NetworkApi | null;
    /// Dispositivos de salida de audio (botón en Controles).
    audio?: AudioApi | null;
    onAudioChanged?: () => void;
    /// Notas rápidas (pestaña Notas).
    notes?: NotesApi | null;
    /// Biblioteca de Wallpaper Engine (pestaña Fondos); sin ella se usa la carpeta de imágenes.
    engine?: EngineLibrary | null;
    onEngineApply?: (id: string, monitor: number | null) => Promise<ActionOutcome>;
    /// Sube cada vez que el panel se muestra: relanza la animación de aparición.
    openTick?: number;
    /// Sube cuando el panel empieza a cerrarse: se reproduce la animación de apertura al revés.
    closeTick?: number;
    /// Alto (lógico) que necesita la pestaña visible: la ventana se ajusta a él.
    onResize?: (height: number) => void;
    onClose: () => void;
    onMediaCommand: (command: MediaCommandName) => void;
    onVolume: (percent: number) => void;
    onMute: (muted: boolean) => void;
    onRadio: (kind: RadioKindName, on: boolean) => Promise<ActionOutcome>;
    onWallpaper: (name: string) => Promise<ActionOutcome>;
  } = $props();

  let shell: HTMLElement | undefined = $state();
  // Abrir relanza la animación de aparición; cerrar la reproduce al revés (el panel se recoge hacia la barra).
  let lastOpen = -1;
  let lastClose = 0;
  $effect(() => {
    const opened = openTick;
    const closed = closeTick;
    if (!shell) return;
    if (opened !== lastOpen) {
      lastOpen = opened;
      lastClose = closed;
      shell.classList.remove("open", "closing");
      void shell.offsetWidth; // fuerza el recálculo para que la animación empiece de nuevo
      shell.classList.add("open");
    } else if (closed !== lastClose) {
      lastClose = closed;
      shell.classList.remove("open");
      void shell.offsetWidth;
      shell.classList.add("closing");
    }
  });

  type TabId = "today" | "controls" | "stats" | "notes" | "modules" | "wallpaper" | "appearance" | "settings";
  const TABS: { id: TabId; label: string }[] = [
    { id: "controls", label: "Controles" },
    { id: "today", label: "Hoy" },
    { id: "stats", label: "Estadísticas" },
    { id: "notes", label: "Notas" },
    { id: "wallpaper", label: "Fondos" },
    { id: "appearance", label: "Personalización" },
    { id: "settings", label: "Ajustes" },
    { id: "modules", label: "Módulos" },
  ];
  let tabId: TabId = $state("today");

  // El marcador de la pestaña activa es una sola pieza que se desliza de una pestaña a otra: el borde por el que
  // sale se mueve más rápido que el otro, así se estira al viajar y se recoge al llegar, como una gota.
  let tabsEl: HTMLElement | undefined = $state();
  let pill = $state({ left: 0, right: 0, ready: false });
  function placePill() {
    const nav = tabsEl;
    const active = nav?.querySelector<HTMLElement>(".tab.active");
    if (!nav || !active) return;
    pill = { left: active.offsetLeft, right: nav.clientWidth - (active.offsetLeft + active.offsetWidth), ready: true };
  }
  $effect(() => {
    tabId;
    visibleTabs;
    tabsEl;
    // Tras pintar las pestañas (la activa ya lleva su clase).
    queueMicrotask(placePill);
  });
  $effect(() => {
    const nav = tabsEl;
    if (!nav || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(placePill);
    ro.observe(nav);
    return () => ro.disconnect();
  });

  // El alto del panel sigue al contenido de la pestaña visible (así no hay barras de desplazamiento): se mide el
  // contenido y se suma lo fijo (borde, márgenes y fila de pestañas). La ventana crece o se encoge a esa medida.
  const CHROME = 82;
  const MIN_HEIGHT = 150;
  const MAX_HEIGHT = 680;
  let measureEl: HTMLElement | undefined = $state();
  let natural = $state(220);
  $effect(() => {
    const el = measureEl;
    if (!el) return;
    natural = el.offsetHeight;
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(() => (natural = el.offsetHeight));
    ro.observe(el);
    return () => ro.disconnect();
  });
  const tabHeight = $derived(Math.min(MAX_HEIGHT, Math.max(MIN_HEIGHT, Math.round(natural + CHROME))));
  // Módulos con sección de panel. Los antiguos de portapapeles, apps y notas ya no existen como módulos: las notas
  // son una pestaña propia, así que si alguien los tiene instalados no se muestran duplicados.
  const dedicated = ["clipboard", "apps", "notes"];
  const otherModules = $derived(modules.filter((m) => !dedicated.includes(m.manifest.id)));
  const visibleTabs = $derived(TABS.filter((t) => t.id !== "modules" || otherModules.some((m) => m.enabled && m.manifest.ui.panel)));

  // Crecer: primero la ventana y luego el panel; encoger: primero el panel y luego la ventana.
  let lastHeight = 0;
  let shrinkTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const h = tabHeight;
    openTick;
    clearTimeout(shrinkTimer);
    if (h >= lastHeight) onResize(h);
    else shrinkTimer = setTimeout(() => onResize(h), 380);
    lastHeight = h;
    return () => clearTimeout(shrinkTimer);
  });

  // «9:05 AM» → ["9", "05", "AM"]
  const clock = $derived(formatClock(now).split(/[: ]/));
  const shortDate = $derived(now.toLocaleDateString("es", { weekday: "short", day: "numeric" }).replace(".", ""));
  const date = $derived(now.toLocaleDateString("es", { weekday: "long", day: "numeric", month: "long" }));
  // De noche el cielo despejado se muestra como luna.
  const night = $derived(now.getHours() < 6 || now.getHours() >= 19);
  // El texto se adapta al fondo: claro sobre oscuro, oscuro sobre claro (tinte del panel + lo que se ve a través).
  let behind: number | null = $state(null);
  const tone = $derived(toneFor(effectiveLuma(behind, 1 - $theme.panelTransparency / 100)));
  const hasModulePanels = $derived(otherModules.some((m) => m.enabled && m.manifest.ui.panel));
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onClose()} />

<div class="shell open" bind:this={shell} style:height={`${tabHeight}px`} style:--fg={tone.fg} style:--muted={tone.muted}>
  <GlassBackdrop kind="panel" blur={$theme.panelBlur} tick={openTick} onLuma={(l) => (behind = l)} />
  <div class="tint" aria-hidden="true"></div>
  <DotField fixed={false} />
  <div class="panel" style:--fg={tone.fg} style:--muted={tone.muted}>
    <nav class="tabs" aria-label="Secciones del panel" bind:this={tabsEl}>
      <span class="pill" class:ready={pill.ready} style:left={`${pill.left}px`} style:right={`${pill.right}px`} aria-hidden="true"></span>
      {#each visibleTabs as t (t.id)}
        <button class="tab" class:active={tabId === t.id} onclick={() => (tabId = t.id)}>{t.label}</button>
      {/each}
    </nav>

    {#key tabId}
      <div class="content">
       <div class="measure" bind:this={measureEl}>
        {#if tabId === "today"}
          <div class="cols today">
            <section class="section clock-card">
              <div class="time">
                <span class="hh">{clock[0]}</span>
                <span class="sep" aria-hidden="true"><i></i><i></i><i></i></span>
                <span class="mm">{clock[1]}</span>
                <span class="ampm">{clock[2]}</span>
              </div>
              <div class="date">{shortDate}</div>
              <div class="weather" aria-label="Clima en Mérida">
                {#if weather}
                  <div class="now-weather" title={describeWeather(weather.code)} role="img" aria-label={describeWeather(weather.code)}><strong>{Math.round(weather.temperature)}°</strong><WeatherIcon code={weather.code} night={night} size={34} /></div>
                  <div class="details">
                    {#if weather.max !== null && weather.min !== null}<span>Máx {Math.round(weather.max)}° · Mín {Math.round(weather.min)}°</span>{/if}
                    {#if weather.feelsLike !== null}<span>Sensación {Math.round(weather.feelsLike)}°</span>{/if}
                  </div>
                  <div class="place">Mérida</div>
                {:else}
                  <div class="hint">Clima de Mérida no disponible.</div>
                {/if}
              </div>
            </section>
            <section class="section week-card">
              <h3>Esta semana <span class="range">{weekRange(now)}</span></h3>
              <div class="days">
                {#each weekDays(now) as d (d.label)}
                  <div class="day" class:today={d.today}><small>{d.label}</small><strong>{d.day}</strong></div>
                {/each}
              </div>
              <div class="date long">{date}</div>
            </section>
            {#if tasks}
              <TasksSection api={tasks} />
            {:else}
              <section class="section"><h3>Tareas</h3><p class="hint">Las tareas no están disponibles.</p></section>
            {/if}
          </div>
        {:else if tabId === "controls"}
          <div class="cols">
            <MediaSection {media} {volume} art={mediaArt} onCommand={onMediaCommand} {onVolume} {onMute} />
            <ConnectivitySection {radios} {allowChanges} onSet={onRadio} {network} {audio} {onAudioChanged} />
          </div>
        {:else if tabId === "appearance"}
          <div class="cols"><section class="section appearance"><AppearancePanel /></section></div>
        {:else if tabId === "settings"}
          <div class="cols"><section class="section settings-section"><SettingsPanel tick={openTick} /></section></div>
        {:else if tabId === "stats"}
          <StatsSection {overview} />
        {:else if tabId === "notes"}
          {#if notes}
            <div class="cols"><NotesSection api={notes} /></div>
          {:else}
            <section class="section"><p class="hint">Las notas no están disponibles.</p></section>
          {/if}
        {:else if tabId === "modules"}
          <div class="cols">
            <ModuleSections modules={otherModules} />
            {#if !hasModulePanels}
              <section class="section"><p class="hint">Ningún módulo instalado añade una sección al panel.</p></section>
            {/if}
          </div>
        {:else if tabId === "wallpaper"}
          <div class="cols">
            {#if engine?.available}
              <EngineWallpapers library={engine} onApply={onEngineApply} />
            {:else}
              <WallpaperSection list={wallpapers} onSet={onWallpaper} />
            {/if}
          </div>
        {/if}
       </div>
      </div>
    {/key}
  </div>
</div>

<style>
  :global(html, body) { margin: 0; height: 100%; background: transparent; overflow: hidden; }
  /* Se despliega desde el centro de la barra: un recorte que se abre hacia abajo y a los lados. */
  .shell {
    position: relative; box-sizing: border-box; overflow: hidden; transition: height 0.38s cubic-bezier(.22, 1, .36, 1);
    border: 1px solid var(--line, rgba(255, 255, 255, 0.14)); border-radius: 22px;
  }
  /* Tinte del cristal: su opacidad es la de Personalización (opaco por defecto). */
  .tint { position: absolute; inset: 0; z-index: 0; pointer-events: none; background: rgba(10, 10, 11, var(--panel-alpha, 1)); }
  .shell.open { animation: drop 0.5s cubic-bezier(.22, 1, .36, 1) both; }
  .shell.open .panel { animation: content-in 0.35s ease 0.12s both; }
  @keyframes drop {
    from { clip-path: inset(0 40% 97% 40% round 22px); opacity: 0; transform: translateY(-8px); }
    35% { opacity: 1; }
    to { clip-path: inset(0 0 0 0 round 22px); opacity: 1; transform: none; }
  }
  .shell.closing { animation: drop 0.38s cubic-bezier(.22, 1, .36, 1) reverse both; pointer-events: none; }
  .shell.closing .panel { animation: content-in 0.2s ease reverse both; }
  @keyframes content-in { from { opacity: 0; transform: translateY(-6px); } to { opacity: 1; transform: none; } }
  .panel {
    position: relative; z-index: 1; box-sizing: border-box; height: 100%;
    display: flex; flex-direction: column; gap: 12px; padding: 14px 18px 18px;
    color: var(--fg); font: 14px/1.3 "Segoe UI", system-ui, sans-serif;
  }
  .tabs { position: relative; display: flex; justify-content: center; align-self: center; gap: 4px; flex: none; }
  .pill { position: absolute; top: 0; bottom: 0; border-radius: 999px; background: var(--accent); box-shadow: 0 0 18px -6px var(--accent); opacity: 0; z-index: 0;
    transition: left 0.5s cubic-bezier(.3, 1.25, .35, 1), right 0.34s cubic-bezier(.3, 1.1, .35, 1), opacity 0.2s; }
  .pill.ready { opacity: 1; }
  .tab { position: relative; z-index: 1; font: 11px var(--mono); letter-spacing: 0.06em; white-space: nowrap; text-transform: uppercase; color: var(--muted); background: transparent; border: 1px solid transparent; border-radius: 999px; padding: 6px 10px; cursor: pointer; transition: color 0.3s, background 0.3s, border-color 0.3s, box-shadow 0.3s; }
  .tab:hover { color: var(--fg); border-color: var(--line); }
  .tab.active { color: var(--on-accent); border-color: transparent; }
  .tab.active:hover { border-color: transparent; }
  .content { flex: 1; min-height: 0; overflow: auto; scrollbar-width: thin; scrollbar-color: rgba(255, 255, 255, 0.2) transparent; animation: tab-in 0.35s cubic-bezier(.2, .8, .2, 1) both; }
  @keyframes tab-in { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: none; } }
  .measure { display: flow-root; }
  .cols { display: grid; grid-template-columns: repeat(auto-fit, minmax(250px, 1fr)); gap: 12px; align-items: start; }
  .cols :global(.section) { margin: 0; }
  .cols.today { grid-template-columns: 0.9fr 1.05fr 1.45fr; }

  .clock-card { text-align: center; display: flex; flex-direction: column; align-items: center; gap: 6px; }
  .time { display: flex; align-items: center; justify-content: center; gap: 10px; }
  .hh, .mm { font: 400 48px/1 var(--mono); letter-spacing: 0.02em; font-variant-numeric: tabular-nums; }
  .ampm { align-self: flex-end; margin-bottom: 6px; font: 600 12px var(--mono); letter-spacing: 0.12em; color: var(--muted); }
  .sep { display: flex; flex-direction: column; gap: 5px; }
  .weather { width: 100%; margin-top: 6px; padding-top: 10px; border-top: 1px dotted var(--line); display: flex; flex-direction: column; gap: 4px; }
  .now-weather { display: flex; align-items: center; justify-content: center; gap: 10px; }
  .now-weather strong { font: 400 28px/1 var(--mono); }
  .now-weather span { color: var(--muted); font: 12px var(--mono); }
  .details { display: flex; flex-wrap: wrap; justify-content: center; gap: 2px 10px; font: 11px var(--mono); color: var(--muted); }
  .place { font: 10px var(--mono); letter-spacing: 0.14em; text-transform: uppercase; opacity: 0.6; }
  .sep i { width: 5px; height: 5px; border-radius: 50%; background: var(--accent); animation: blink 2s steps(20) infinite; }
  .sep i:nth-child(2) { animation-delay: 0.25s; }
  .sep i:nth-child(3) { animation-delay: 0.5s; }
  @keyframes blink { 50% { opacity: 0.2; } }
  .date { color: var(--muted); font: 11px var(--mono); text-transform: uppercase; letter-spacing: 0.12em; }
  .date.long { margin-top: 10px; }
  .range { margin-left: auto; font-weight: 400; letter-spacing: 0.06em; text-transform: none; }
  .week-card h3 { display: flex; }
  .days { display: grid; grid-template-columns: repeat(7, 1fr); gap: 6px; text-align: center; }
  .day { padding: 8px 0; border-radius: 14px; display: flex; flex-direction: column; gap: 4px; transition: background 0.3s; }
  .day small { opacity: 0.6; font: 10px var(--mono); text-transform: uppercase; }
  .day strong { font: 500 15px var(--mono); }
  .day.today { background: var(--accent); color: var(--on-accent); box-shadow: 0 0 16px -6px var(--accent); }
  @media (prefers-reduced-motion: reduce) { .shell.open, .shell.open .panel, .shell.closing, .shell.closing .panel, .content { animation: none; } }
</style>
