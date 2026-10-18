import { cleanup, render } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

import GlassBackdrop from "./GlassBackdrop.svelte";

const glass = { url: "data:image/bmp;base64,Qk0=", width: 940, height: 600 };

beforeEach(() => {
  vi.useFakeTimers();
  invoke.mockReset();
  invoke.mockImplementation((cmd: string) => Promise.resolve(cmd === "glass_image" ? glass : undefined));
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

const flush = () => vi.advanceTimersByTimeAsync(400);
const imageCalls = () => invoke.mock.calls.filter((c) => c[0] === "glass_image");

describe("GlassBackdrop", () => {
  it("la barra pide la imagen con la intensidad elegida y la dibuja con su tamaño", async () => {
    const { container } = render(GlassBackdrop, { kind: "bar", blur: 40 });
    await flush();
    expect(imageCalls()).toEqual([["glass_image", { kind: "bar", blur: 40, refresh: false }]]);
    const img = container.querySelector("img.glass") as HTMLImageElement;
    expect(img.getAttribute("src")).toBe(glass.url);
    expect(img.style.width).toBe("940px");
    expect(img.style.height).toBe("600px");
  });

  it("sin desenfoque no pide nada: se ve lo de detrás tal cual", async () => {
    const { container } = render(GlassBackdrop, { kind: "bar", blur: 0 });
    await flush();
    expect(imageCalls()).toEqual([]);
    expect(container.querySelector("img.glass")).toBeNull();
  });

  it("al mover el deslizador espera a que se detenga y pide una sola vez", async () => {
    const { rerender } = render(GlassBackdrop, { kind: "bar", blur: 10 });
    await flush();
    invoke.mockClear();
    for (const v of [20, 30, 45, 60]) {
      await rerender({ blur: v });
      await vi.advanceTimersByTimeAsync(40);
    }
    expect(imageCalls()).toEqual([]);
    await flush();
    expect(imageCalls()).toEqual([["glass_image", { kind: "bar", blur: 60, refresh: false }]]);
  });

  it("al volver a poner el desenfoque en cero se quita la imagen", async () => {
    const { container, rerender } = render(GlassBackdrop, { kind: "bar", blur: 40 });
    await flush();
    expect(container.querySelector("img.glass")).toBeTruthy();
    await rerender({ blur: 0 });
    await flush();
    expect(container.querySelector("img.glass")).toBeNull();
  });

  it("si el núcleo no pudo copiar lo de detrás, queda solo el tinte", async () => {
    invoke.mockImplementation(() => Promise.reject("no se pudo"));
    const { container } = render(GlassBackdrop, { kind: "bar", blur: 40 });
    await flush();
    expect(container.querySelector("img.glass")).toBeNull();
  });

  it("el panel pide la copia cada vez que se abre y al subir el desenfoque desde cero", async () => {
    const { rerender, container } = render(GlassBackdrop, { kind: "panel", blur: 50, tick: 0 });
    await flush();
    expect(imageCalls().length).toBe(1);
    await rerender({ tick: 1 });
    await flush();
    expect(imageCalls().length).toBe(2);
    expect(imageCalls()[1][1]).toEqual({ kind: "panel", blur: 50, refresh: false });
    await rerender({ blur: 0 });
    await flush();
    expect(container.querySelector("img.glass")).toBeNull();
    const calls = imageCalls().length;
    await rerender({ blur: 30 });
    await flush();
    expect(imageCalls().length).toBe(calls + 1);
  });

  it("el panel abierto vuelve a copiar el fondo cuando cambia el fondo de pantalla", async () => {
    render(GlassBackdrop, { kind: "panel", blur: 40 });
    await flush();
    invoke.mockClear();
    window.dispatchEvent(new Event("wm-wallpaper-changed"));
    await flush();
    expect(imageCalls()).toEqual([["glass_image", { kind: "panel", blur: 40, refresh: true }]]);
  });

  it("la barra vuelve a copiar el fondo cuando cambia el fondo de pantalla", async () => {
    render(GlassBackdrop, { kind: "bar", blur: 40 });
    await flush();
    invoke.mockClear();
    window.dispatchEvent(new Event("wm-wallpaper-changed"));
    await flush();
    expect(imageCalls()).toEqual([["glass_image", { kind: "bar", blur: 40, refresh: true }]]);
  });
});
