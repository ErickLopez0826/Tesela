import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { tick } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Task, TasksApi } from "../api/tasks";
import PanelView from "./PanelView.svelte";

afterEach(cleanup);

const noop = vi.fn();
const base = {
  now: new Date(2026, 5, 1, 9, 5, 7),
  overview: null,
  media: null,
  volume: null,
  radios: [],
  wallpapers: null,
  allowChanges: false,
  onClose: noop,
  onMediaCommand: noop,
  onVolume: noop,
  onMute: noop,
  onRadio: vi.fn().mockResolvedValue({ simulated: true, description: "" }),
  onWallpaper: vi.fn().mockResolvedValue({ simulated: false, description: "" }),
};

const tasksApi = (list: Task[] = []): TasksApi => ({ list: vi.fn().mockResolvedValue(list), save: vi.fn(async (t) => t) });

describe("PanelView", () => {
  it("abre en «Hoy» con la hora en 12 horas y las tareas, sin el módulo de workspaces", () => {
    const { container } = render(PanelView, { ...base, tasks: tasksApi() });
    expect(container.querySelector(".hh")?.textContent).toBe("9");
    expect(container.querySelector(".mm")?.textContent).toBe("05");
    expect(container.querySelector(".ampm")?.textContent).toBe("AM");
    // Reloj en horizontal: la hora a la izquierda y los minutos a la derecha.
    const order = [...container.querySelector(".time")!.children].map((c) => (c.classList.contains("hh") ? "hh" : c.classList.contains("mm") ? "mm" : ""));
    expect(order.indexOf("hh")).toBeLessThan(order.indexOf("mm"));
    expect(order.indexOf("hh")).toBeGreaterThanOrEqual(0);
    expect(screen.getByText("Tareas")).toBeTruthy();
    expect(screen.queryByText("Workspaces")).toBeNull();
    expect(container.querySelector(".ws")).toBeNull();
    expect(screen.getByText("Esta semana", { exact: false })).toBeTruthy();
  });

  it("«Hoy» abre un calendario del mes con hoy marcado, navega entre meses y se cierra", async () => {
    const { container } = render(PanelView, { ...base });
    expect(screen.queryByRole("grid")).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "Abrir calendario" }));
    expect(screen.getByText("Calendario")).toBeTruthy();
    expect(screen.getByRole("grid", { name: "Junio de 2026" })).toBeTruthy();
    expect(container.querySelector(".day")).toBeNull();
    expect(container.querySelector(".cell.today")?.textContent).toBe("1");

    await fireEvent.click(screen.getByRole("button", { name: "Mes siguiente" }));
    expect(screen.getByRole("grid", { name: "Julio de 2026" })).toBeTruthy();
    expect(container.querySelector(".cell.today")).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "Mes anterior" }));
    await fireEvent.click(screen.getByRole("button", { name: "Mes anterior" }));
    expect(screen.getByRole("grid", { name: "Mayo de 2026" })).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Volver al mes de hoy" }));
    expect(screen.getByRole("grid", { name: "Junio de 2026" })).toBeTruthy();

    await fireEvent.click(screen.getByRole("button", { name: "Cerrar calendario" }));
    expect(screen.queryByRole("grid")).toBeNull();
    expect(container.querySelectorAll(".day")).toHaveLength(7);
  });

  it("«Hoy» muestra el clima de Mérida y avisa si no está disponible", () => {
    const { unmount } = render(PanelView, { ...base, weather: { temperature: 31.4, feelsLike: 36, humidity: 62, wind: 11, code: 2, max: 34, min: 25 } });
    expect(screen.getByText("31°")).toBeTruthy();
    // El estado del tiempo es un icono (con su nombre como etiqueta), no un texto.
    const icon = screen.getByRole("img", { name: "Parcialmente nublado" });
    expect(icon.querySelector("svg")?.getAttribute("data-kind")).toBe("partly-sun");
    expect(screen.queryByText("Parcialmente nublado")).toBeNull();
    expect(screen.queryByText(/Humedad|Viento/)).toBeNull();
    unmount();
    render(PanelView, { ...base });
    expect(screen.getByText("Clima de Mérida no disponible.")).toBeTruthy();
  });

  it("incluye la pestaña Personalización y no hay pestaña Audio", async () => {
    render(PanelView, { ...base });
    expect(screen.queryByRole("button", { name: "Audio" })).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "Personalización" }));
    expect(screen.getByText("Modo de color")).toBeTruthy();
  });

  it("tiene pestaña Notas y ya no tiene Portapapeles ni Apps", async () => {
    render(PanelView, { ...base });
    expect(screen.queryByRole("button", { name: "Portapapeles" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Apps" })).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "Notas" }));
    expect(screen.getByText("Las notas no están disponibles.")).toBeTruthy();
  });

  it("hay un único marcador de pestaña que se desliza", () => {
    const { container } = render(PanelView, { ...base });
    expect(container.querySelectorAll(".pill").length).toBe(1);
    expect(container.querySelectorAll(".tab.active").length).toBe(1);
  });

  it("reúne todas las secciones del panel (§9.2) en sus pestañas", async () => {
    render(PanelView, { ...base });
    const tabs: [string, string | RegExp][] = [
      ["Controles", "Medios"],
      ["Controles", "Conectividad"],
      ["Estadísticas", "Estadísticas"],
      ["Fondos", "Fondos de pantalla"],
    ];
    for (const [tab, content] of tabs) {
      await fireEvent.click(screen.getByRole("button", { name: tab }));
      // Con la pestaña activa, además del botón aparece el título de la sección.
      expect(screen.getAllByText(content).length).toBeGreaterThan(0);
    }
  });

  it("se cierra con Escape", async () => {
    const onClose = vi.fn();
    render(PanelView, { ...base, onClose });
    await fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalled();
  });

  it("«Hoy» enseña las tareas guardadas y avisa si no están disponibles", async () => {
    const { unmount } = render(PanelView, { ...base, tasks: tasksApi([{ id: "1", text: "Llamar al banco", priority: "high" }]) });
    expect(await screen.findByText("Llamar al banco")).toBeTruthy();
    unmount();
    render(PanelView, base);
    expect(screen.getByText("Las tareas no están disponibles.")).toBeTruthy();
  });

});

describe("PanelView: cierre", () => {
  it("al subir closeTick el panel se cierra con la animación al revés, y al abrir de nuevo vuelve a abrirse", async () => {
    const view = render(PanelView, { ...base, openTick: 1, closeTick: 0 });
    const shell = view.container.querySelector(".shell")!;
    expect(shell.classList.contains("closing")).toBe(false);
    await view.rerender({ ...base, openTick: 1, closeTick: 1 });
    expect(shell.classList.contains("closing")).toBe(true);
    expect(shell.classList.contains("open")).toBe(false);
    await view.rerender({ ...base, openTick: 2, closeTick: 1 });
    await tick();
    expect(shell.classList.contains("closing")).toBe(false);
    expect(shell.classList.contains("open")).toBe(true);
  });
});
