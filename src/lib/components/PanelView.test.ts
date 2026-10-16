import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { tick } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { StateSnapshot } from "../api/state";
import PanelView from "./PanelView.svelte";

afterEach(cleanup);

const noop = vi.fn();
const base = {
  state: null as StateSnapshot | null,
  now: new Date(2026, 5, 1, 9, 5, 7),
  overview: null,
  media: null,
  volume: null,
  radios: [],
  wallpapers: null,
  allowChanges: false,
  onWorkspace: noop,
  onClose: noop,
  onMediaCommand: noop,
  onVolume: noop,
  onMute: noop,
  onRadio: vi.fn().mockResolvedValue({ simulated: true, description: "" }),
  onPower: vi.fn().mockResolvedValue({ simulated: true, description: "" }),
  onWallpaper: vi.fn().mockResolvedValue({ simulated: false, description: "" }),
};

const state: StateSnapshot = {
  monitors: [],
  workspaces: Array.from({ length: 9 }, (_, i) => ({ number: i + 1, monitor: 1, visible: i === 0, active: i === 0, window_count: i })),
  focused: null,
  paused: false,
  mode: null,
};

describe("PanelView", () => {
  it("abre en «Hoy» con la hora en 12 horas y los workspaces", () => {
    const { container } = render(PanelView, { ...base, state });
    expect(container.querySelector(".hh")?.textContent).toBe("9");
    expect(container.querySelector(".mm")?.textContent).toBe("05");
    expect(container.querySelector(".ampm")?.textContent).toBe("AM");
    // Reloj en horizontal: la hora a la izquierda y los minutos a la derecha.
    const order = [...container.querySelector(".time")!.children].map((c) => (c.classList.contains("hh") ? "hh" : c.classList.contains("mm") ? "mm" : ""));
    expect(order.indexOf("hh")).toBeLessThan(order.indexOf("mm"));
    expect(order.indexOf("hh")).toBeGreaterThanOrEqual(0);
    expect(screen.getByText("Workspaces")).toBeTruthy();
    expect(screen.getByText("Esta semana", { exact: false })).toBeTruthy();
  });

  it("«Hoy» muestra el clima de Mérida y avisa si no está disponible", () => {
    const { unmount } = render(PanelView, { ...base, state, weather: { temperature: 31.4, feelsLike: 36, humidity: 62, wind: 11, code: 2, max: 34, min: 25 } });
    expect(screen.getByText("31°")).toBeTruthy();
    // El estado del tiempo es un icono (con su nombre como etiqueta), no un texto.
    const icon = screen.getByRole("img", { name: "Parcialmente nublado" });
    expect(icon.querySelector("svg")?.getAttribute("data-kind")).toBe("partly-sun");
    expect(screen.queryByText("Parcialmente nublado")).toBeNull();
    expect(screen.queryByText(/Humedad|Viento/)).toBeNull();
    unmount();
    render(PanelView, { ...base, state });
    expect(screen.getByText("Clima de Mérida no disponible.")).toBeTruthy();
  });

  it("incluye la pestaña Personalización y no hay pestaña Audio", async () => {
    render(PanelView, { ...base, state });
    expect(screen.queryByRole("button", { name: "Audio" })).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "Personalización" }));
    expect(screen.getByText("Modo de color")).toBeTruthy();
  });

  it("tiene pestaña Notas y ya no tiene Portapapeles ni Apps", async () => {
    render(PanelView, { ...base, state });
    expect(screen.queryByRole("button", { name: "Portapapeles" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Apps" })).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "Notas" }));
    expect(screen.getByText("Las notas no están disponibles.")).toBeTruthy();
  });

  it("hay un único marcador de pestaña que se desliza", () => {
    const { container } = render(PanelView, { ...base, state });
    expect(container.querySelectorAll(".pill").length).toBe(1);
    expect(container.querySelectorAll(".tab.active").length).toBe(1);
  });

  it("reúne todas las secciones del panel (§9.2) en sus pestañas", async () => {
    render(PanelView, { ...base, state });
    const tabs: [string, string | RegExp][] = [
      ["Controles", "Medios"],
      ["Controles", "Conectividad"],
      ["Estadísticas", "Estadísticas"],
      ["Fondos", "Fondos de pantalla"],
      ["Energía", /Energía/],
    ];
    for (const [tab, content] of tabs) {
      await fireEvent.click(screen.getByRole("button", { name: tab }));
      // Con la pestaña activa, además del botón aparece el título de la sección.
      expect(screen.getAllByText(content).length).toBeGreaterThan(0);
    }
  });

  it("cambia de workspace desde el panel y se cierra con Escape", async () => {
    const onWorkspace = vi.fn();
    const onClose = vi.fn();
    const { container } = render(PanelView, { ...base, state, onWorkspace, onClose });
    await fireEvent.click(container.querySelectorAll(".ws")[2]);
    expect(onWorkspace).toHaveBeenCalledWith(3);
    await fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalled();
  });

  it("explica que el gestor no está activo", () => {
    render(PanelView, base);
    expect(screen.getByText("El gestor de ventanas no está activo.")).toBeTruthy();
  });
});

describe("PanelView: cierre", () => {
  it("al subir closeTick el panel se cierra con la animación al revés, y al abrir de nuevo vuelve a abrirse", async () => {
    const view = render(PanelView, { ...base, state, openTick: 1, closeTick: 0 });
    const shell = view.container.querySelector(".shell")!;
    expect(shell.classList.contains("closing")).toBe(false);
    await view.rerender({ ...base, state, openTick: 1, closeTick: 1 });
    expect(shell.classList.contains("closing")).toBe(true);
    expect(shell.classList.contains("open")).toBe(false);
    await view.rerender({ ...base, state, openTick: 2, closeTick: 1 });
    await tick();
    expect(shell.classList.contains("closing")).toBe(false);
    expect(shell.classList.contains("open")).toBe(true);
  });
});
