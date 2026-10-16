import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { StateSnapshot } from "../api/state";
import BarView from "./BarView.svelte";

afterEach(cleanup);

const state: StateSnapshot = {
  monitors: [{ id: 1, primary: true, vertical: false, visible_workspace: 1, bar_visible: true }],
  workspaces: [
    { number: 1, monitor: 1, visible: true, active: true, window_count: 2 },
    { number: 2, monitor: 1, visible: false, active: false, window_count: 0 },
    { number: 3, monitor: 1, visible: false, active: false, window_count: 1 },
    { number: 5, monitor: 2, visible: true, active: false, window_count: 0 },
  ],
  focused: { exe: "code.exe", title: "Mi proyecto" },
  paused: false,
  mode: null,
};
const stats = { cpu_percent: 37.4, ram_used: 8 * 1024 ** 3, ram_total: 16 * 1024 ** 3 };
const now = new Date(2026, 5, 1, 9, 5, 7);

describe("BarView", () => {
  it("muestra workspaces del monitor, ventana enfocada, CPU, RAM y reloj", () => {
    render(BarView, { monitorId: 1, state, stats, now, onWorkspace: vi.fn() });
    expect(screen.getByText("1")).toBeTruthy();
    expect(screen.getByText("3")).toBeTruthy();
    expect(screen.queryByText("2")).toBeNull();
    expect(screen.queryByText("5")).toBeNull();
    expect(screen.getByText("Mi proyecto")).toBeTruthy();
    expect(screen.getByText("CPU 37%")).toBeTruthy();
    expect(screen.getByText("RAM 50%")).toBeTruthy();
    expect(screen.getByText("9:05 AM")).toBeTruthy();
  });

  it("cambia de workspace con un clic y ya no tiene botón de panel (lo abre la isla)", async () => {
    const onWorkspace = vi.fn();
    render(BarView, { monitorId: 1, state, stats, now, onWorkspace });
    await fireEvent.click(screen.getByText("3"));
    expect(onWorkspace).toHaveBeenCalledWith(3);
    expect(screen.queryByLabelText("Abrir panel")).toBeNull();
  });

  it("funciona sin estado del gestor (solo reloj)", () => {
    render(BarView, { monitorId: 1, state: null, stats: null, now, onWorkspace: vi.fn() });
    expect(screen.getByText("9:05 AM")).toBeTruthy();
  });

  it("avisa de pausa y modo", () => {
    render(BarView, { monitorId: 1, state: { ...state, paused: true, mode: "resize" }, stats, now, onWorkspace: vi.fn() });
    expect(screen.getByText("en pausa")).toBeTruthy();
    expect(screen.getByText("modo resize")).toBeTruthy();
  });
});

describe("BarView: separadores, pastilla y workspaces nuevos", () => {
  it("separa los datos con tres puntos, sin separadores huérfanos", () => {
    const full = render(BarView, { monitorId: 1, state, stats, now, volume: { percent: 40, muted: false }, gpu: 10, temperature: 30, onWorkspace: vi.fn() });
    // CPU, RAM, GPU, temperatura, volumen y hora: seis datos, cinco separadores.
    expect(full.container.querySelectorAll(".dots[aria-hidden='true']").length).toBe(5);
    cleanup();
    const bare = render(BarView, { monitorId: 1, state: null, stats: null, now, onWorkspace: vi.fn() });
    expect(bare.container.querySelectorAll(".dots").length).toBe(0);
  });

  it("la pastilla sigue al workspace activo y se oculta si el activo es de otro monitor", () => {
    const own = render(BarView, { monitorId: 1, state, stats, now, onWorkspace: vi.fn() });
    expect(own.container.querySelector(".pill.ready")).not.toBeNull();
    cleanup();
    const other = render(BarView, { monitorId: 2, state, stats, now, onWorkspace: vi.fn() });
    expect(other.container.querySelector(".pill.ready")).toBeNull();
  });

  it("un workspace que aparece después entra animado; los iniciales no", async () => {
    const view = render(BarView, { monitorId: 1, state, stats, now, onWorkspace: vi.fn() });
    expect(view.container.querySelector(".ws.enter")).toBeNull();
    const withTwo = { ...state, workspaces: [state.workspaces[0], { number: 2, monitor: 1, visible: false, active: false, window_count: 1 }, state.workspaces[2]] };
    await view.rerender({ monitorId: 1, state: withTwo, stats, now, onWorkspace: vi.fn() });
    const entered = Array.from(view.container.querySelectorAll(".ws.enter")).map((e) => e.textContent?.trim());
    expect(entered).toEqual(["2"]);
  });

  it("el título va dentro de la caja con desplazamiento", () => {
    const view = render(BarView, { monitorId: 1, state, stats, now, onWorkspace: vi.fn() });
    expect(view.container.querySelector(".title .marquee")?.textContent).toContain("Mi proyecto");
  });
});

describe("BarView: volumen, gráfica y temperatura de Mérida", () => {
  it("muestra el volumen, la gráfica y la temperatura, sin la canción, y silencia con un clic", async () => {
    const onMute = vi.fn();
    render(BarView, {
      monitorId: 1,
      state,
      stats,
      now,
      volume: { percent: 40, muted: false },
      gpu: 37,
      temperature: 61.4,
      onWorkspace: vi.fn(),
      onMute,
    });
    expect(screen.queryByText("Banda – Tema")).toBeNull();
    expect(screen.getByText(/^GPU 37/)).toBeTruthy();
    expect(screen.getByText("Mérida 61 °C")).toBeTruthy();
    await fireEvent.click(screen.getByLabelText("Silenciar"));
    expect(onMute).toHaveBeenCalledWith(true);
  });
});
