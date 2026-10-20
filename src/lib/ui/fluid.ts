/**
 * Efecto «líquido» de los botones: una luz suave que sigue al cursor por dentro del botón y, al pulsar, una onda que se
 * expande desde el punto del clic. Se instala una sola vez por ventana con delegación de eventos (no hay que marcar
 * cada botón); los estilos están en `fluid.css`.
 */
const TARGET = "button, .fluid";

function target(e: Event): HTMLElement | null {
  const el = e.target;
  if (!(el instanceof Element)) return null;
  return el.closest<HTMLElement>(TARGET);
}

/**
 * Al pulsar, los botones se encogen un poco (`:active`). Si el clic cae en su borde, el puntero queda fuera del botón ya encogido:
 * el navegador no lo da por un clic y la acción se pierde aunque se viera la animación. Este rescate lo detecta: si se suelta el botón
 * del ratón dentro de donde estaba el botón al pulsarlo, y no sobre otro botón, se hace el clic que faltaba.
 */
export function missedClick(pressed: HTMLButtonElement | null, rect: DOMRect | null, releasedOn: Element | null, x: number, y: number): boolean {
  if (!pressed || !rect || !pressed.isConnected || pressed.disabled) return false;
  const on = releasedOn?.closest("button") ?? null;
  // Sobre el propio botón el clic llega solo; sobre otro botón, la persona se movió allí.
  if (on) return false;
  return x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
}

export function installFluid(doc: Document = document): () => void {
  let pressed: HTMLButtonElement | null = null;
  let pressedRect: DOMRect | null = null;

  const remember = (e: PointerEvent) => {
    const t = target(e);
    const button = t instanceof HTMLButtonElement && !t.disabled && e.button === 0 ? t : null;
    pressed = button;
    pressedRect = button ? button.getBoundingClientRect() : null;
  };

  const release = (e: PointerEvent) => {
    const button = pressed;
    const rect = pressedRect;
    pressed = null;
    pressedRect = null;
    if (e.button === 0 && missedClick(button, rect, e.target instanceof Element ? e.target : null, e.clientX, e.clientY)) button?.click();
  };

  const cancel = () => {
    pressed = null;
    pressedRect = null;
  };

  const move = (e: PointerEvent) => {
    const t = target(e);
    if (!t) return;
    const r = t.getBoundingClientRect();
    t.style.setProperty("--fx", `${e.clientX - r.left}px`);
    t.style.setProperty("--fy", `${e.clientY - r.top}px`);
  };

  const down = (e: PointerEvent) => {
    remember(e);
    const t = target(e);
    if (!t || (t instanceof HTMLButtonElement && t.disabled)) return;
    if (matchMedia?.("(prefers-reduced-motion: reduce)").matches) return;
    const r = t.getBoundingClientRect();
    const size = Math.max(r.width, r.height) * 2.6;
    const ink = doc.createElement("span");
    ink.className = "ink";
    ink.style.width = ink.style.height = `${size}px`;
    ink.style.left = `${e.clientX - r.left - size / 2}px`;
    ink.style.top = `${e.clientY - r.top - size / 2}px`;
    t.appendChild(ink);
    const anim = ink.animate(
      [
        { transform: "scale(0.05)", opacity: 0.45, borderRadius: "50%" },
        { transform: "scale(0.7)", opacity: 0.25, borderRadius: "46% 54% 52% 48% / 55% 45% 55% 45%" },
        { transform: "scale(1)", opacity: 0, borderRadius: "50%" },
      ],
      { duration: 720, easing: "cubic-bezier(.2,.8,.2,1)" },
    );
    anim.onfinish = () => ink.remove();
    anim.oncancel = () => ink.remove();
  };

  doc.addEventListener("pointermove", move, { passive: true });
  doc.addEventListener("pointerdown", down, { passive: true });
  doc.addEventListener("pointerup", release, { passive: true });
  doc.addEventListener("pointercancel", cancel, { passive: true });
  return () => {
    doc.removeEventListener("pointerup", release);
    doc.removeEventListener("pointercancel", cancel);
    doc.removeEventListener("pointermove", move);
    doc.removeEventListener("pointerdown", down);
  };
}
