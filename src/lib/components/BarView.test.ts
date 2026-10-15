import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { StateSnapshot } from "../api/state";
import BarView from "./BarView.svelte";
import PanelView from "./PanelView.svelte";

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
    render(BarView, { monitorId: 1, state, stats, now, onWorkspace: vi.fn(), onPanel: vi.fn() });
    expect(screen.getByText("1")).toBeTruthy();
    expect(screen.getByText("3")).toBeTruthy();
    expect(screen.queryByText("2")).toBeNull();
    expect(screen.queryByText("5")).toBeNull();
    expect(screen.getByText("Mi proyecto")).toBeTruthy();
    expect(screen.getByText("CPU 37%")).toBeTruthy();
    expect(screen.getByText("RAM 50%")).toBeTruthy();
    expect(screen.getByText("09:05")).toBeTruthy();
  });

  it("cambia de workspace y abre el panel con un clic", async () => {
    const onWorkspace = vi.fn();
    const onPanel = vi.fn();
    render(BarView, { monitorId: 1, state, stats, now, onWorkspace, onPanel });
    await fireEvent.click(screen.getByText("3"));
    expect(onWorkspace).toHaveBeenCalledWith(3);
    await fireEvent.click(screen.getByLabelText("Abrir panel"));
    expect(onPanel).toHaveBeenCalled();
  });

  it("funciona sin estado del gestor (solo reloj)", () => {
    render(BarView, { monitorId: 1, state: null, stats: null, now, onWorkspace: vi.fn(), onPanel: vi.fn() });
    expect(screen.getByText("09:05")).toBeTruthy();
  });

  it("avisa de pausa y modo", () => {
    render(BarView, { monitorId: 1, state: { ...state, paused: true, mode: "resize" }, stats, now, onWorkspace: vi.fn(), onPanel: vi.fn() });
    expect(screen.getByText("en pausa")).toBeTruthy();
    expect(screen.getByText("modo resize")).toBeTruthy();
  });
});

describe("PanelView", () => {
  it("muestra reloj, CPU, RAM y los 9 workspaces", () => {
    const all = Array.from({ length: 9 }, (_, i) => ({ number: i + 1, monitor: 1, visible: i === 0, active: i === 0, window_count: i }));
    render(PanelView, { state: { ...state, workspaces: all }, stats, now, onWorkspace: vi.fn(), onClose: vi.fn() });
    expect(screen.getByText("09:05:07")).toBeTruthy();
    expect(screen.getByText("37%")).toBeTruthy();
    expect(screen.getByText("8.0 GB de 16.0 GB")).toBeTruthy();
    expect(screen.getAllByRole("button")).toHaveLength(9);
  });

  it("cambia de workspace y se cierra con Escape", async () => {
    const onWorkspace = vi.fn();
    const onClose = vi.fn();
    render(PanelView, { state, stats, now, onWorkspace, onClose });
    await fireEvent.click(screen.getByText("3").closest("button")!);
    expect(onWorkspace).toHaveBeenCalledWith(3);
    await fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalled();
  });

  it("explica que el gestor no está activo", () => {
    render(PanelView, { state: null, stats: null, now, onWorkspace: vi.fn(), onClose: vi.fn() });
    expect(screen.getByText("El gestor de ventanas no está activo.")).toBeTruthy();
  });
});
