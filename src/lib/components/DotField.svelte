<!-- Fondo monocromo de puntos: los que quedan bajo el cursor se iluminan en un área pequeña y la luz lo sigue con
     suavidad. Al pulsar un botón, una ola de luz recorre los puntos desde ese punto hacia fuera, y cuando cambia el color de Tesela
     los puntos nuevos avanzan como una ola de agua sobre los anteriores. Decorativo. -->
<script lang="ts">
  import { onMount } from "svelte";

  // `fixed`: cubre la ventana entera; si es falso, rellena el contenedor posicionado que lo contiene.
  let { fixed = true }: { fixed?: boolean } = $props();

  const RECOLOR_MS = 1500;
  const readDot = () => getComputedStyle(document.documentElement).getPropertyValue("--dot").trim() || "255, 255, 255";
  const reduced = () => typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;

  let field: HTMLElement | undefined = $state();
  /** Color («r, g, b») con el que se dibujan los puntos ahora. */
  let shown = $state(readDot());
  /** Color nuevo que avanza en forma de ola sobre el anterior mientras dura el cambio. */
  let incoming: string | null = $state(null);
  let recolor: Animation | undefined;
  let lastDown = { x: 0, y: 0, at: 0 };

  function move(e: PointerEvent) {
    if (!field) return;
    const r = field.getBoundingClientRect();
    field.style.setProperty("--mx", `${e.clientX - r.left}px`);
    field.style.setProperty("--my", `${e.clientY - r.top}px`);
    field.style.setProperty("--lit", "1");
  }

  function leave() {
    field?.style.setProperty("--lit", "0");
  }

  /** Distancia del punto a la esquina más lejana: hasta ahí tiene que crecer una ola para cubrirlo todo. */
  function reachFrom(x: number, y: number, w: number, h: number) {
    return Math.hypot(Math.max(x, w - x), Math.max(y, h - y)) + 90;
  }

  /** Un anillo de puntos encendidos que se expande desde (x, y) y se apaga al alejarse. */
  function ring(x: number, y: number, duration: number, color?: string) {
    if (!field) return;
    const r = field.getBoundingClientRect();
    const el = document.createElement("div");
    el.className = "dots ripple";
    el.style.setProperty("--wx", `${x}px`);
    el.style.setProperty("--wy", `${y}px`);
    if (color) el.style.setProperty("--dot", color);
    field.appendChild(el);
    const anim = el.animate(
      [
        { "--wr": "0px", opacity: 1 },
        { opacity: 1, offset: 0.55 },
        { "--wr": `${reachFrom(x, y, r.width, r.height)}px`, opacity: 0 },
      ],
      { duration, easing: "cubic-bezier(.25,.6,.3,1)" },
    );
    anim.onfinish = anim.oncancel = () => el.remove();
  }

  /** Al pulsar un botón, una ola de luz recorre los puntos desde ese punto hacia fuera. */
  function wave(e: PointerEvent) {
    if (!field) return;
    const r = field.getBoundingClientRect();
    lastDown = { x: e.clientX - r.left, y: e.clientY - r.top, at: Date.now() };
    if (!(e.target instanceof Element) || !e.target.closest("button, .fluid") || reduced()) return;
    ring(lastDown.x, lastDown.y, 1100);
  }

  function settle() {
    recolor?.cancel();
    recolor = undefined;
    if (incoming) shown = incoming;
    incoming = null;
  }

  /**
   * El color de Tesela cambió: los puntos nuevos avanzan como una ola de agua sobre los anteriores. Con un botón nace donde
   * se pulsó; con un cambio de fondo de pantalla, en el centro.
   */
  function recolored(e: Event) {
    if (!field) return;
    const next = readDot();
    if (next === (incoming ?? shown)) return;
    settle();
    if (next === shown) return;
    if (reduced() || typeof field.animate !== "function") {
      shown = next;
      return;
    }
    const r = field.getBoundingClientRect();
    const byButton = (e as CustomEvent).detail?.source !== "wallpaper" && Date.now() - lastDown.at < 1500;
    const [x, y] = byButton ? [lastDown.x, lastDown.y] : [r.width / 2, r.height / 2];
    field.style.setProperty("--rx", `${x}px`);
    field.style.setProperty("--ry", `${y}px`);
    incoming = next;
    const reach = reachFrom(x, y, r.width, r.height);
    ring(x, y, RECOLOR_MS, next);
    recolor = field.animate([{ "--rr": "0px" }, { "--rr": `${reach}px` }], { duration: RECOLOR_MS, easing: "cubic-bezier(.25,.6,.3,1)" });
    recolor.onfinish = () => {
      if (incoming) shown = incoming;
      incoming = null;
      recolor = undefined;
    };
  }

  onMount(() => {
    window.addEventListener("wm-theme-applied", recolored);
    return () => {
      window.removeEventListener("wm-theme-applied", recolored);
      recolor?.cancel();
    };
  });
</script>

<svelte:window onpointermove={move} onpointerdowncapture={wave} onblur={leave} onmouseout={(e) => !e.relatedTarget && leave()} />

<div class="field" class:contained={!fixed} class:recoloring={incoming !== null} style:--dot={shown} bind:this={field} aria-hidden="true">
  <div class="dots base old"></div>
  {#if incoming}<div class="dots base new" style:--dot={incoming}></div>{/if}
  <div class="dots lit"></div>
</div>

<style>
  /* Las coordenadas del cursor son propiedades registradas: al cambiar se animan, así la luz sigue al cursor con
     un pequeño retraso elástico en vez de saltar. */
  @property --mx { syntax: "<length>"; inherits: true; initial-value: -200px; }
  @property --my { syntax: "<length>"; inherits: true; initial-value: -200px; }
  @property --lit { syntax: "<number>"; inherits: true; initial-value: 0; }
  @property --wr { syntax: "<length>"; inherits: true; initial-value: 0px; }
  @property --rr { syntax: "<length>"; inherits: true; initial-value: 0px; }

  .field {
    position: fixed; inset: 0; z-index: -1; overflow: hidden; pointer-events: none;
    background: var(--bg, #0a0a0b);
    transition: --mx 0.22s cubic-bezier(.2, .9, .3, 1), --my 0.22s cubic-bezier(.2, .9, .3, 1), --lit 0.5s ease;
  }
  /* Dentro del panel no pinta fondo: el tinte (y lo que se ve a través) lo pone el propio panel. */
  .field.contained { position: absolute; z-index: 0; background: transparent; }
  .field :global(.dots) { position: absolute; inset: 0; background-size: 18px 18px; }
  .base { background-image: radial-gradient(circle, rgba(var(--dot, 255, 255, 255), 0.14) 1px, transparent 1.4px); }
  .lit {
    background-image: radial-gradient(circle, rgba(var(--dot, 255, 255, 255), 0.95) 1.3px, transparent 1.8px);
    opacity: var(--lit);
    -webkit-mask-image: radial-gradient(circle 76px at var(--mx) var(--my), #000 0%, rgba(0, 0, 0, 0.45) 45%, transparent 100%);
    mask-image: radial-gradient(circle 76px at var(--mx) var(--my), #000 0%, rgba(0, 0, 0, 0.45) 45%, transparent 100%);
  }
  /* Cambio de color: los puntos viejos quedan fuera del círculo que crece (--rr) y los nuevos dentro, con un borde suave. */
  .field.recoloring :global(.old) {
    -webkit-mask-image: radial-gradient(circle at var(--rx) var(--ry), transparent calc(var(--rr) - 30px), #000 var(--rr));
    mask-image: radial-gradient(circle at var(--rx) var(--ry), transparent calc(var(--rr) - 30px), #000 var(--rr));
  }
  .field :global(.new) {
    -webkit-mask-image: radial-gradient(circle at var(--rx) var(--ry), #000 calc(var(--rr) - 30px), transparent var(--rr));
    mask-image: radial-gradient(circle at var(--rx) var(--ry), #000 calc(var(--rr) - 30px), transparent var(--rr));
  }
  /* El anillo de la ola: un borde suave de unos 90 px de ancho que crece con --wr. */
  .field :global(.ripple) {
    background-image: radial-gradient(circle, rgba(var(--dot, 255, 255, 255), 1) 1.5px, transparent 2.1px);
    -webkit-mask-image: radial-gradient(circle at var(--wx) var(--wy), transparent calc(var(--wr) - 90px), #000 calc(var(--wr) - 22px), #000 calc(var(--wr) - 8px), transparent var(--wr));
    mask-image: radial-gradient(circle at var(--wx) var(--wy), transparent calc(var(--wr) - 90px), #000 calc(var(--wr) - 22px), #000 calc(var(--wr) - 8px), transparent var(--wr));
  }
  @media (prefers-reduced-motion: reduce) { .field { transition: none; } }
</style>
