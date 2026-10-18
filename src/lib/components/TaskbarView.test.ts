import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { TaskbarApi } from "../api/shell";
import type { StateSnapshot, WindowSnapshot } from "../api/state";
import { clearIconCache } from "../stores/taskbar";
import TaskbarView from "./TaskbarView.svelte";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn().mockResolvedValue(null) }));

const win = (id: number, workspace: number, exe: string, extra: Partial<WindowSnapshot> = {}): WindowSnapshot => ({
  id,
  workspace,
  title: `${exe} ${id}`,
  exe,
  focused: false,
  minimized: false,
  ...extra,
});

function snapshot(windows: WindowSnapshot[]): StateSnapshot {
  return {
    monitors: [],
    workspaces: [
      { number: 1, monitor: 0, visible: true, active: true, window_count: 0 },
      { number: 2, monitor: 0, visible: false, active: false, window_count: 0 },
      { number: 3, monitor: 0, visible: false, active: false, window_count: 0 },
    ],
    focused: null,
    paused: false,
    mode: null,
    windows,
  };
}

function api(): TaskbarApi {
  return {
    focus: vi.fn().mockResolvedValue(undefined),
    icon: vi.fn().mockResolvedValue("data:image/png;base64,AAA"),
    resize: vi.fn().mockResolvedValue(undefined),
    hide: vi.fn().mockResolvedValue(undefined),
  };
}

beforeEach(() => clearIconCache());
afterEach(cleanup);

describe("TaskbarView", () => {
  it("muestra las apps separadas por workspace, en orden", () => {
    render(TaskbarView, { state: snapshot([win(1, 1, "code"), win(2, 3, "chrome"), win(3, 3, "spotify")]), monitorId: 0, api: api(), onWorkspace: () => {} });
    const groups = [...document.querySelectorAll(".group")].map((g) => [g.getAttribute("aria-label"), g.querySelectorAll(".app").length]);
    expect(groups).toEqual([
      ["Workspace 1", 1],
      ["Workspace 3", 2],
    ]);
  });

  it("tocar una app la enfoca y cierra la barra; tocar el número va a ese workspace y la deja abierta", async () => {
    const a = api();
    const onWorkspace = vi.fn();
    render(TaskbarView, { state: snapshot([win(7, 3, "chrome")]), monitorId: 0, api: a, onWorkspace });
    await fireEvent.click(screen.getByRole("button", { name: "Ir al workspace 3" }));
    expect(onWorkspace).toHaveBeenCalledWith(3);
    expect(a.hide).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole("button", { name: "chrome 7" }));
    expect(a.focus).toHaveBeenCalledWith(7);
    expect(a.hide).toHaveBeenCalledTimes(1);
  });

  it("el despliegue y el cierre del núcleo reproducen la animación (aparece y se cierra)", async () => {
    const { rerender } = render(TaskbarView, { state: snapshot([]), monitorId: 0, api: api(), onWorkspace: () => {}, openTick: 0, closeTick: 0 });
    const stage = document.querySelector(".stage")!;
    expect(stage.classList.contains("closing")).toBe(false);
    await rerender({ closeTick: 1 });
    expect(stage.classList.contains("closing")).toBe(true);
    await rerender({ openTick: 1 });
    expect(stage.classList.contains("closing")).toBe(false);
    expect(stage.classList.contains("alt")).toBe(true);
    await rerender({ openTick: 2 });
    expect(stage.classList.contains("alt")).toBe(false);
  });

  it("la ventana con el foco y la minimizada se distinguen", () => {
    render(TaskbarView, { state: snapshot([win(1, 1, "a", { focused: true }), win(2, 1, "b", { minimized: true })]), monitorId: 0, api: api(), onWorkspace: () => {} });
    expect(screen.getByRole("button", { name: "a 1" }).classList.contains("focused")).toBe(true);
    const min = screen.getByRole("button", { name: "b 2 (minimizada)" });
    expect(min.classList.contains("minimized")).toBe(true);
  });

  it("dibuja el icono que da el núcleo y, mientras llega, la letra del programa", async () => {
    render(TaskbarView, { state: snapshot([win(1, 1, "code")]), monitorId: 0, api: api(), onWorkspace: () => {} });
    expect(document.querySelector(".fallback")?.textContent).toBe("C");
    await waitFor(() => expect(document.querySelector(".app img")?.getAttribute("src")).toBe("data:image/png;base64,AAA"));
  });

  it("si no hay icono se queda con la letra", async () => {
    const a = api();
    a.icon = vi.fn().mockResolvedValue(null);
    render(TaskbarView, { state: snapshot([win(1, 1, "spotify")]), monitorId: 0, api: a, onWorkspace: () => {} });
    await waitFor(() => expect(a.icon).toHaveBeenCalled());
    await Promise.resolve();
    expect(document.querySelector(".app img")).toBeNull();
    expect(document.querySelector(".fallback")?.textContent).toBe("S");
  });

  it("sin apps solo queda el workspace visible, y sin estado no revienta", () => {
    render(TaskbarView, { state: snapshot([]), monitorId: 0, api: api(), onWorkspace: () => {} });
    expect(document.querySelectorAll(".group").length).toBe(1);
    cleanup();
    render(TaskbarView, { state: null, monitorId: 0, api: api(), onWorkspace: () => {} });
    expect(document.querySelectorAll(".group").length).toBe(0);
  });
});
