<!-- Buscador de aplicaciones (§18): un cuadro de texto y, debajo, los resultados. Mismo cristal y animaciones que el panel. -->
<script lang="ts">
  import "../styles/panel.css";
  import { onMount } from "svelte";
  import type { SearchApi, SearchItem } from "../api/search";
  import { errorText, hasIcon, letter, moveSelection } from "../stores/search";
  import { effectiveLuma, toneFor } from "../stores/tone";
  import { theme } from "../stores/theme";
  import DotField from "./DotField.svelte";
  import GlassBackdrop from "./GlassBackdrop.svelte";
  import Icon from "./Icon.svelte";
  import SiteIcon from "./SiteIcon.svelte";

  let {
    api,
    openTick = 0,
    closeTick = 0,
  }: {
    api: SearchApi;
    /** Sube cada vez que el buscador se muestra: relanza la animación, vacía el cuadro y lo enfoca. */
    openTick?: number;
    /** Sube cuando el buscador empieza a cerrarse: se reproduce la animación de apertura al revés. */
    closeTick?: number;
  } = $props();

  let shell: HTMLElement | undefined = $state();
  let content: HTMLElement | undefined = $state();
  let input: HTMLInputElement | undefined = $state();
  let list: HTMLElement | undefined = $state();
  let text = $state("");
  let items: SearchItem[] = $state([]);
  let selected = $state(-1);
  let error: string | null = $state(null);
  /** Iconos ya pedidos, por nombre: los identificadores cambian en cada consulta, los nombres no. */
  let icons: Record<string, string> = $state({});
  const asked = new Set<string>();
  let seq = 0;

  // Abrir relanza la animación de aparición; cerrar la reproduce al revés (la ventana se recoge hacia la barra).
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

  // La ventana se reutiliza: cada vez que se abre, el cuadro empieza vacío (con lo más usado debajo) y enfocado.
  let lastReset = -1;
  $effect(() => {
    const opened = openTick;
    if (opened === lastReset) return;
    lastReset = opened;
    text = "";
    void run("");
    input?.focus();
  });

  async function run(q: string) {
    const mine = ++seq;
    try {
      const found = await api.query(q);
      if (mine !== seq) return; // llegó una consulta más nueva: esta ya no sirve
      items = found;
      selected = found.length ? 0 : -1;
      error = null;
      for (const item of found) {
        if (!hasIcon(item) || asked.has(item.title)) continue;
        asked.add(item.title);
        api
          .icon(item.id)
          .then((url) => {
            if (url) icons[item.title] = url;
            else asked.delete(item.title);
          })
          .catch(() => asked.delete(item.title));
      }
    } catch (e) {
      if (mine !== seq) return;
      items = [];
      selected = -1;
      error = errorText(e);
    }
  }

  function open(item: SearchItem | undefined, asAdmin: boolean) {
    if (!item) return;
    error = null;
    api.open(item.id, asAdmin).catch((e) => (error = errorText(e)));
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      void api.close();
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      selected = moveSelection(selected, e.key === "ArrowDown" ? 1 : -1, items.length);
    } else if (e.key === "Enter") {
      e.preventDefault();
      open(items[selected], e.ctrlKey);
    }
  }

  // El elemento elegido con las flechas siempre queda a la vista.
  $effect(() => {
    const el = list?.children[selected] as HTMLElement | undefined;
    el?.scrollIntoView?.({ block: "nearest" });
  });

  // La ventana se ajusta al alto de su contenido (el borde de 1 px de cada lado se suma).
  let lastHeight = 0;
  function report() {
    if (!content) return;
    const height = Math.ceil(content.offsetHeight) + 2;
    if (height > 2 && height !== lastHeight) {
      lastHeight = height;
      void api.resize(height)?.catch?.(() => {});
    }
  }
  onMount(() => {
    report();
    if (typeof ResizeObserver === "undefined" || !content) return;
    const observer = new ResizeObserver(report);
    observer.observe(content);
    return () => observer.disconnect();
  });

  // El texto se adapta al fondo: claro sobre oscuro, oscuro sobre claro (tinte + lo que se ve a través).
  let behind: number | null = $state(null);
  const tone = $derived(toneFor(effectiveLuma(behind, 1 - $theme.panelTransparency / 100)));
</script>

<svelte:window onkeydown={onKey} onfocus={() => input?.focus()} />

<div class="shell open" bind:this={shell} style:--fg={tone.fg} style:--muted={tone.muted}>
  <GlassBackdrop kind="search" blur={$theme.panelBlur} tick={openTick} onLuma={(l) => (behind = l)} />
  <div class="tint" aria-hidden="true"></div>
  <DotField fixed={false} />
  <div class="panel" bind:this={content} style:--fg={tone.fg} style:--muted={tone.muted}>
    <div class="box">
      <span class="lens" aria-hidden="true"><Icon name="search" size={18} /></span>
      <input
        bind:this={input}
        bind:value={text}
        oninput={() => void run(text)}
        type="text"
        role="combobox"
        aria-label="Buscar"
        aria-expanded={items.length > 0}
        aria-controls="search-results"
        aria-activedescendant={selected >= 0 ? `search-result-${selected}` : undefined}
        aria-autocomplete="list"
        placeholder="Buscar aplicaciones o escribir una palabra…"
        autocomplete="off"
        spellcheck="false"
      />
    </div>

    {#if items.length > 0}
      <ul id="search-results" class="results" role="listbox" aria-label="Resultados" bind:this={list}>
        {#each items as item, i}
          <!-- mousedown (y no click): así el cuadro de texto no pierde el foco antes de elegir -->
          <li
            id={`search-result-${i}`}
            role="option"
            aria-selected={i === selected}
            class:active={i === selected}
            onmousemove={() => (selected = i)}
            onmousedown={(e) => (e.preventDefault(), open(item, e.ctrlKey))}
          >
            <span class="ico">
              {#if icons[item.title]}
                <img src={icons[item.title]} alt="" draggable="false" />
              {:else if item.site}
                <SiteIcon site={item.site} fallback={item.kind === "web_search" ? "search" : "globe"} />
              {:else if item.kind === "web_search"}
                <Icon name="search" size={18} />
              {:else if item.kind === "redirect"}
                <Icon name="globe" size={18} />
              {:else}
                <span class="fallback">{letter(item.title)}</span>
              {/if}
            </span>
            <span class="text">
              <span class="title">{item.title}</span>
              <span class="sub">{item.subtitle}</span>
            </span>
            {#if item.kind === "running"}<span class="live" title="En ejecución" aria-hidden="true"></span>{/if}
          </li>
        {/each}
      </ul>
      <p class="keys">↑↓ elegir · Enter abrir · Ctrl+Enter como administrador · Esc cerrar</p>
    {:else if text.trim() && !error}
      <p class="none">Sin resultados para «{text.trim()}»</p>
    {/if}
    {#if error}<p class="problem" role="alert">{error}</p>{/if}
  </div>
</div>

<style>
  :global(html, body) { margin: 0; height: 100%; background: transparent; overflow: hidden; }
  .shell {
    position: relative; box-sizing: border-box; overflow: hidden;
    border: 1px solid var(--line, rgba(255, 255, 255, 0.14)); border-radius: 22px;
  }
  /* Tinte del cristal: su opacidad es la de Personalización, igual que el panel. */
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
    position: relative; z-index: 1; box-sizing: border-box; display: flex; flex-direction: column; gap: 8px; padding: 12px;
    color: var(--fg); font: 14px/1.3 "Segoe UI", system-ui, sans-serif;
  }

  .box { display: flex; align-items: center; gap: 10px; padding: 0 14px; height: 52px; border-radius: 16px; background: rgba(0, 0, 0, 0.35); border: 1px solid var(--line); transition: border-color 0.3s; }
  .box:focus-within { border-color: var(--accent); }
  .lens { display: grid; place-items: center; color: var(--muted); }
  input { flex: 1; min-width: 0; font: 17px "Segoe UI", system-ui, sans-serif; color: var(--fg); background: transparent; border: 0; outline: none; padding: 0; }
  input::placeholder { color: var(--muted); }

  /* Unas 8 filas a la vista; con más se desplaza, con la barra de desplazamiento oculta. */
  .results { list-style: none; margin: 0; padding: 0; max-height: 416px; overflow-y: auto; scrollbar-width: none; display: flex; flex-direction: column; gap: 2px; }
  .results::-webkit-scrollbar { display: none; }
  li { display: flex; align-items: center; gap: 12px; height: 50px; box-sizing: border-box; padding: 0 12px; border-radius: 14px; cursor: pointer; flex: none; border: 1px solid transparent; }
  li.active { background: rgba(255, 255, 255, 0.12); border-color: var(--line); }
  .ico { display: grid; place-items: center; flex: none; width: 32px; height: 32px; color: var(--muted); }
  .ico img { width: 28px; height: 28px; object-fit: contain; user-select: none; }
  .fallback { display: grid; place-items: center; width: 28px; height: 28px; border-radius: 8px; background: rgba(255, 255, 255, 0.1); font: 600 13px var(--mono); color: var(--fg); }
  .text { display: flex; flex-direction: column; min-width: 0; flex: 1; }
  .title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 14px; }
  .sub { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font: 11px var(--mono); color: var(--muted); }
  .live { flex: none; width: 7px; height: 7px; border-radius: 50%; background: var(--accent); box-shadow: 0 0 8px var(--accent); }
  .keys, .none { margin: 2px 4px 0; font: 10px var(--mono); letter-spacing: 0.04em; color: var(--muted); text-align: center; }
  .none { font-size: 12px; padding: 10px 0 6px; }
  .problem { margin: 2px 4px 0; color: #ff5a5f; font-size: 12px; }
  @media (prefers-reduced-motion: reduce) { .shell.open, .shell.open .panel, .shell.closing, .shell.closing .panel { animation: none; } }
</style>
