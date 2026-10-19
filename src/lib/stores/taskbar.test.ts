import { beforeEach, describe, expect, it, vi } from "vitest";
import type { StateSnapshot, WindowSnapshot } from "../api/state";
import { clearIconCache, groupsFor, iconKey, loadIcon, windowLabel } from "./taskbar";

const win = (id: number, workspace: number, exe = "app", extra: Partial<WindowSnapshot> = {}): WindowSnapshot => ({
  id,
  workspace,
  title: `ventana ${id}`,
  exe,
  focused: false,
  minimized: false,
  ...extra,
});

const ws = (number: number, monitor: number, visible = false, active = false) => ({ number, monitor, visible, active, window_count: 0 });

function state(windows: WindowSnapshot[]): StateSnapshot {
  return {
    monitors: [],
    workspaces: [ws(1, 0, true, true), ws(2, 0), ws(3, 0), ws(4, 1, true), ws(5, 1)],
    focused: null,
    paused: false,
    mode: null,
    windows,
  };
}

beforeEach(() => clearIconCache());

describe("barra de tareas: grupos", () => {
  it("agrupa las ventanas por workspace, en orden, y salta los que no tienen nada", () => {
    const groups = groupsFor(state([win(1, 1), win(2, 3), win(3, 3), win(4, 2)]), 0);
    expect(groups.map((g) => [g.workspace, g.windows.map((w) => w.id)])).toEqual([
      [1, [1]],
      [2, [4]],
      [3, [2, 3]],
    ]);
  });

  it("solo los workspaces de ese monitor; el visible aparece aunque esté vacío", () => {
    const s = state([win(1, 2)]);
    expect(groupsFor(s, 0).map((g) => g.workspace)).toEqual([1, 2]);
    expect(groupsFor(s, 1).map((g) => g.workspace)).toEqual([4]);
    expect(groupsFor(s, 9)).toEqual([]);
  });

  it("marca el workspace activo y el visible", () => {
    const [first] = groupsFor(state([win(1, 1)]), 0);
    expect([first.active, first.visible]).toEqual([true, true]);
  });

  it("sin estado o sin lista de ventanas no hay nada que dibujar salvo el workspace visible", () => {
    expect(groupsFor(null, 0)).toEqual([]);
    const s = state([]);
    delete s.windows;
    expect(groupsFor(s, 0).map((g) => g.workspace)).toEqual([1]);
  });
});

describe("barra de tareas: iconos", () => {
  it("las ventanas del mismo programa comparten icono; las apps de la tienda, no", () => {
    expect(iconKey(win(1, 1, "chrome"))).toBe(iconKey(win(2, 1, "chrome")));
    expect(iconKey(win(1, 1, "applicationframehost"))).not.toBe(iconKey(win(2, 1, "applicationframehost")));
  });

  it("el icono se pide una sola vez por clave", async () => {
    const ask = vi.fn().mockResolvedValue("data:image/png;base64,AAA");
    await loadIcon(win(1, 1, "chrome"), ask);
    await loadIcon(win(2, 1, "chrome"), ask);
    expect(ask).toHaveBeenCalledTimes(1);
    expect(ask).toHaveBeenCalledWith(1, "chrome");
  });

  it("un icono que no se pudo leer se vuelve a intentar", async () => {
    const ask = vi.fn().mockRejectedValueOnce("falló").mockResolvedValue("data:x");
    expect(await loadIcon(win(1, 1, "code"), ask)).toBeNull();
    await Promise.resolve();
    expect(await loadIcon(win(1, 1, "code"), ask)).toBe("data:x");
  });

  it("la etiqueta es el título, o el programa si no tiene", () => {
    expect(windowLabel(win(1, 1, "code", { title: "  Mi archivo " }))).toBe("Mi archivo");
    expect(windowLabel(win(1, 1, "code", { title: " " }))).toBe("code");
  });
});

describe("backgroundLabel", () => {
  it("resume nombre, CPU y memoria, y cuenta los procesos si hay varios", async () => {
    const { backgroundLabel } = await import("./taskbar");
    const bytes = (n: number) => `${n / 1e6} MB`;
    expect(backgroundLabel({ name: "Discord", cpu_percent: 1.234, ram_bytes: 200e6, processes: 6 }, bytes)).toBe("Discord · CPU 1.2% · 200 MB · 6 procesos");
    expect(backgroundLabel({ name: "Steam", cpu_percent: 0, ram_bytes: 100e6, processes: 1 }, bytes)).toBe("Steam · CPU 0% · 100 MB");
    expect(backgroundLabel({ name: "X", cpu_percent: 12.6, ram_bytes: 1e6, processes: 1 }, bytes)).toContain("CPU 13%");
  });
});
