import { afterEach, describe, expect, it, vi } from "vitest";
import { installFluid, missedClick } from "./fluid";

const rect = (left: number, top: number, right: number, bottom: number) => ({ left, top, right, bottom, width: right - left, height: bottom - top, x: left, y: top, toJSON() {} }) as DOMRect;

describe("missedClick", () => {
  const button = () => {
    const b = document.createElement("button");
    document.body.appendChild(b);
    return b;
  };
  afterEach(() => (document.body.innerHTML = ""));

  it("rescata el clic cuando se suelta dentro de donde estaba el botón y ya no hay botón bajo el puntero", () => {
    const b = button();
    expect(missedClick(b, rect(0, 0, 100, 30), document.body, 99, 15)).toBe(true);
    expect(missedClick(b, rect(0, 0, 100, 30), document.body, 100, 30)).toBe(true);
  });

  it("no hace nada si se suelta fuera del botón, sobre él (el clic llega solo) o sobre otro botón", () => {
    const b = button();
    const other = button();
    expect(missedClick(b, rect(0, 0, 100, 30), document.body, 101, 15)).toBe(false);
    expect(missedClick(b, rect(0, 0, 100, 30), document.body, 50, 31)).toBe(false);
    expect(missedClick(b, rect(0, 0, 100, 30), b, 50, 15)).toBe(false);
    expect(missedClick(b, rect(0, 0, 100, 30), other, 50, 15)).toBe(false);
  });

  it("no hace nada sin botón pulsado, desactivado o ya quitado", () => {
    const b = button();
    expect(missedClick(null, rect(0, 0, 100, 30), document.body, 50, 15)).toBe(false);
    expect(missedClick(b, null, document.body, 50, 15)).toBe(false);
    b.disabled = true;
    expect(missedClick(b, rect(0, 0, 100, 30), document.body, 50, 15)).toBe(false);
    b.disabled = false;
    b.remove();
    expect(missedClick(b, rect(0, 0, 100, 30), document.body, 50, 15)).toBe(false);
  });
});

describe("installFluid: clic en el borde de un botón que se encoge al pulsarlo", () => {
  let stop: (() => void) | null = null;
  afterEach(() => {
    stop?.();
    stop = null;
    document.body.innerHTML = "";
  });

  const pointer = (type: string, target: Element, x: number, y: number, button = 0) =>
    target.dispatchEvent(new MouseEvent(type, { bubbles: true, clientX: x, clientY: y, button }));

  function setup() {
    // jsdom no trae animaciones: la onda del clic se simula.
    Element.prototype.animate = vi.fn(() => ({}) as Animation);
    stop = installFluid(document);
    const b = document.createElement("button");
    const onclick = vi.fn();
    b.onclick = onclick;
    b.getBoundingClientRect = () => rect(0, 0, 100, 30);
    document.body.appendChild(b);
    return { b, onclick };
  }

  it("si el puntero queda fuera del botón ya encogido, el clic se hace igual", () => {
    const { b, onclick } = setup();
    pointer("pointerdown", b, 99, 15);
    pointer("pointerup", document.body, 99, 15); // el botón encogido ya no está bajo el puntero
    expect(onclick).toHaveBeenCalledTimes(1);
  });

  it("un clic normal no se duplica y soltar fuera no hace clic", () => {
    const { b, onclick } = setup();
    pointer("pointerdown", b, 50, 15);
    pointer("pointerup", b, 50, 15);
    expect(onclick).not.toHaveBeenCalled(); // el clic real lo manda el navegador, no el rescate
    pointer("pointerdown", b, 99, 15);
    pointer("pointerup", document.body, 140, 15);
    expect(onclick).not.toHaveBeenCalled();
  });

  it("solo con el botón izquierdo", () => {
    const { b, onclick } = setup();
    pointer("pointerdown", b, 99, 15, 2);
    pointer("pointerup", document.body, 99, 15, 2);
    expect(onclick).not.toHaveBeenCalled();
  });
});
