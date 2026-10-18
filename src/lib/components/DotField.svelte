<!-- Fondo monocromo de puntos: los que quedan bajo el cursor se iluminan en un área pequeña y la luz lo sigue con
     suavidad. Al pulsar un botón, una ola de luz recorre los puntos desde ese punto hacia fuera. Decorativo. -->
<script lang="ts">
  // `fixed`: cubre la ventana entera; si es falso, rellena el contenedor posicionado que lo contiene.
  let { fixed = true }: { fixed?: boolean } = $props();

  let field: HTMLElement | undefined = $state();

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

  /** Ola: un anillo de puntos encendidos que se expande desde el botón pulsado y se apaga al alejarse. */
  function wave(e: PointerEvent) {
    if (!field || !(e.target instanceof Element) || !e.target.closest("button, .fluid")) return;
    if (matchMedia?.("(prefers-reduced-motion: reduce)").matches) return;
    const r = field.getBoundingClientRect();
    const el = document.createElement("div");
    el.className = "dots ripple";
    el.style.setProperty("--wx", `${e.clientX - r.left}px`);
    el.style.setProperty("--wy", `${e.clientY - r.top}px`);
    field.appendChild(el);
    const reach = Math.hypot(Math.max(e.clientX - r.left, r.width - (e.clientX - r.left)), Math.max(e.clientY - r.top, r.height - (e.clientY - r.top))) + 90;
    const anim = el.animate(
      [
        { "--wr": "0px", opacity: 1 },
        { opacity: 1, offset: 0.55 },
        { "--wr": `${reach}px`, opacity: 0 },
      ],
      { duration: 1100, easing: "cubic-bezier(.25,.6,.3,1)" },
    );
    anim.onfinish = anim.oncancel = () => el.remove();
  }
</script>

<svelte:window onpointermove={move} onpointerdowncapture={wave} onblur={leave} onmouseout={(e) => !e.relatedTarget && leave()} />

<div class="field" class:contained={!fixed} bind:this={field} aria-hidden="true">
  <div class="dots base"></div>
  <div class="dots lit"></div>
</div>

<style>
  /* Las coordenadas del cursor son propiedades registradas: al cambiar se animan, así la luz sigue al cursor con
     un pequeño retraso elástico en vez de saltar. */
  @property --mx { syntax: "<length>"; inherits: true; initial-value: -200px; }
  @property --my { syntax: "<length>"; inherits: true; initial-value: -200px; }
  @property --lit { syntax: "<number>"; inherits: true; initial-value: 0; }
  @property --wr { syntax: "<length>"; inherits: true; initial-value: 0px; }

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
  /* El anillo de la ola: un borde suave de unos 90 px de ancho que crece con --wr. */
  .field :global(.ripple) {
    background-image: radial-gradient(circle, rgba(var(--dot, 255, 255, 255), 1) 1.5px, transparent 2.1px);
    -webkit-mask-image: radial-gradient(circle at var(--wx) var(--wy), transparent calc(var(--wr) - 90px), #000 calc(var(--wr) - 22px), #000 calc(var(--wr) - 8px), transparent var(--wr));
    mask-image: radial-gradient(circle at var(--wx) var(--wy), transparent calc(var(--wr) - 90px), #000 calc(var(--wr) - 22px), #000 calc(var(--wr) - 8px), transparent var(--wr));
  }
  @media (prefers-reduced-motion: reduce) { .field { transition: none; } }
</style>
