import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
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
  it("reúne todas las secciones del panel (§9.2)", () => {
    render(PanelView, { ...base, state });
    expect(screen.getByText("09:05:07")).toBeTruthy();
    for (const heading of ["Medios", "Estadísticas", "Workspaces", "Conectividad", "Fondos de pantalla"]) {
      expect(screen.getByText(heading)).toBeTruthy();
    }
    expect(screen.getByText(/Energía/)).toBeTruthy();
  });

  it("cambia de workspace desde el panel y se cierra con Escape", async () => {
    const onWorkspace = vi.fn();
    const onClose = vi.fn();
    render(PanelView, { ...base, state, onWorkspace, onClose });
    await fireEvent.click(screen.getByText("3").closest("button")!);
    expect(onWorkspace).toHaveBeenCalledWith(3);
    await fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalled();
  });

  it("explica que el gestor no está activo", () => {
    render(PanelView, base);
    expect(screen.getByText("El gestor de ventanas no está activo.")).toBeTruthy();
  });
});
