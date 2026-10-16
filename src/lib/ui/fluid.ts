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

export function installFluid(doc: Document = document): () => void {
  const move = (e: PointerEvent) => {
    const t = target(e);
    if (!t) return;
    const r = t.getBoundingClientRect();
    t.style.setProperty("--fx", `${e.clientX - r.left}px`);
    t.style.setProperty("--fy", `${e.clientY - r.top}px`);
  };

  const down = (e: PointerEvent) => {
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
  return () => {
    doc.removeEventListener("pointermove", move);
    doc.removeEventListener("pointerdown", down);
  };
}
