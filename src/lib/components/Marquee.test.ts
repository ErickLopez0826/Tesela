import { cleanup, render } from "@testing-library/svelte";
import { afterEach, describe, expect, it } from "vitest";
import Marquee from "./Marquee.svelte";

afterEach(() => {
  cleanup();
  delete (HTMLElement.prototype as unknown as Record<string, unknown>).scrollWidth;
  delete (HTMLElement.prototype as unknown as Record<string, unknown>).clientWidth;
});

function sizes(scroll: number, client: number) {
  Object.defineProperty(HTMLElement.prototype, "scrollWidth", { configurable: true, get: () => scroll });
  Object.defineProperty(HTMLElement.prototype, "clientWidth", { configurable: true, get: () => client });
}

describe("Marquee", () => {
  it("si el texto cabe se queda quieto", () => {
    sizes(100, 200);
    const { container } = render(Marquee, { text: "Corto" });
    expect(container.querySelector(".scrolls")).toBeNull();
    expect(container.textContent).toContain("Corto");
  });

  it("si no cabe se desplaza lo que desborda, con un mínimo de duración", () => {
    sizes(500, 200);
    const { container } = render(Marquee, { text: "Un título larguísimo" });
    const text = container.querySelector<HTMLElement>(".scrolls .text");
    expect(text).not.toBeNull();
    expect(text!.style.getPropertyValue("--shift")).toBe("300px");
    expect(parseFloat(text!.style.getPropertyValue("--dur"))).toBeGreaterThanOrEqual(4);
  });
});
