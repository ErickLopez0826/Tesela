import { cleanup, render } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import DotField from "./DotField.svelte";

type Fake = { onfinish: (() => void) | null; oncancel: (() => void) | null; cancel: () => void };
let animations: { target: Element; frames: Keyframe[]; anim: Fake }[] = [];

beforeEach(() => {
  animations = [];
  document.documentElement.style.setProperty("--dot", "255, 255, 255");
  Element.prototype.animate = function (this: Element, frames: Keyframe[] | PropertyIndexedKeyframes | null) {
    const anim: Fake = { onfinish: null, oncancel: null, cancel: () => anim.oncancel?.() };
    animations.push({ target: this, frames: frames as Keyframe[], anim });
    return anim as unknown as Animation;
  };
});
afterEach(() => {
  cleanup();
  document.documentElement.style.removeProperty("--dot");
});

const change = (rgb: string, source: "theme" | "wallpaper") => {
  document.documentElement.style.setProperty("--dot", rgb);
  window.dispatchEvent(new CustomEvent("wm-theme-applied", { detail: { source } }));
};

describe("DotField: ola de color", () => {
  it("al cambiar el color, los puntos nuevos entran como una ola y al terminar quedan solos", async () => {
    const { container } = render(DotField);
    expect(container.querySelector(".new")).toBeNull();
    change("255, 0, 0", "wallpaper");
    await Promise.resolve();
    expect(container.querySelector(".new")).not.toBeNull();
    expect(container.querySelector(".field")?.classList.contains("recoloring")).toBe(true);
    const grow = animations.find((a) => a.target === container.querySelector(".field"))!;
    grow.anim.onfinish?.();
    await Promise.resolve();
    expect(container.querySelector(".new")).toBeNull();
    expect((container.querySelector(".field") as HTMLElement).style.getPropertyValue("--dot")).toBe("255, 0, 0");
  });

  it("con un cambio de fondo de pantalla la ola nace en el centro, no donde se hizo clic", async () => {
    const { container } = render(DotField);
    const field = container.querySelector(".field") as HTMLElement;
    field.getBoundingClientRect = () => ({ left: 0, top: 0, width: 400, height: 200, right: 400, bottom: 200, x: 0, y: 0, toJSON() {} }) as DOMRect;
    window.dispatchEvent(new PointerEvent("pointerdown", { clientX: 30, clientY: 20 }));
    change("0, 200, 0", "wallpaper");
    await Promise.resolve();
    expect(field.style.getPropertyValue("--rx")).toBe("200px");
    expect(field.style.getPropertyValue("--ry")).toBe("100px");
  });

  it("con un botón de color la ola nace donde se pulsó", async () => {
    const { container } = render(DotField);
    const field = container.querySelector(".field") as HTMLElement;
    field.getBoundingClientRect = () => ({ left: 0, top: 0, width: 400, height: 200, right: 400, bottom: 200, x: 0, y: 0, toJSON() {} }) as DOMRect;
    window.dispatchEvent(new PointerEvent("pointerdown", { clientX: 30, clientY: 20 }));
    change("0, 0, 255", "theme");
    await Promise.resolve();
    expect(field.style.getPropertyValue("--rx")).toBe("30px");
    expect(field.style.getPropertyValue("--ry")).toBe("20px");
  });

  it("si el color no cambia no hay ola", async () => {
    const { container } = render(DotField);
    change("255, 255, 255", "wallpaper");
    await Promise.resolve();
    expect(container.querySelector(".new")).toBeNull();
  });
});

