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
    exeIcon: vi.fn().mockResolvedValue("data:image/png;base64,BBB"),
    openBackground: vi.fn().mockResolvedValue(undefined),
    openPopup: vi.fn().mockResolvedValue(undefined),
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

describe("TaskbarView: segundo plano y energía", () => {
  // jsdom no tiene `Element.animate`, que usan las transiciones de Svelte al aparecer iconos nuevos.
  beforeEach(() => {
    (Element.prototype as unknown as { animate: unknown }).animate ??= () => ({ finished: Promise.resolve(), cancel() {}, onfinish: null });
  });
  const bg = (n: number) => ({ name: `App ${n}`, exe: `app${n}.exe`, path: `C:/app${n}.exe`, cpu_percent: 1.5, ram_bytes: 100e6, processes: 1 });

  it("las apps en segundo plano están ocultas y se despliegan hacia arriba al pulsar el botón", async () => {
    const a = api();
    render(TaskbarView, { state: snapshot([]), monitorId: 0, api: a, onWorkspace: () => {}, background: [bg(1), bg(2)] });
    expect(document.querySelector(".bgpanel")).toBeNull();
    const toggle = screen.getByRole("button", { name: "Aplicaciones en segundo plano (2)" });
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    await fireEvent.click(toggle);
    expect(document.querySelectorAll(".bgpanel .card").length).toBe(2);
    const card = screen.getByRole("button", { name: /App 1 · CPU 1.5%/ });
    expect(card.textContent).toContain("App 1");
    await waitFor(() => expect(card.querySelector("img")?.getAttribute("src")).toBe("data:image/png;base64,BBB"));
    await fireEvent.click(card);
    expect(a.openBackground).toHaveBeenCalledWith("app1.exe");
    expect(a.hide).toHaveBeenCalledTimes(1);
    await fireEvent.click(toggle);
    expect(document.querySelector(".bgpanel")).toBeNull();
  });

  it("«⋯» abre la ventana emergente de esa app junto al botón y no despliega nada dentro de la barra", async () => {
    const a = api();
    render(TaskbarView, { state: snapshot([]), monitorId: 0, api: a, onWorkspace: () => {}, background: [bg(1)] });
    await fireEvent.click(screen.getByRole("button", { name: "Aplicaciones en segundo plano (1)" }));
    const before = document.querySelector(".bgpanel")!.innerHTML.length;
    await fireEvent.click(screen.getByRole("button", { name: "Acciones de App 1" }));
    expect(a.openPopup).toHaveBeenCalledTimes(1);
    expect((a.openPopup as ReturnType<typeof vi.fn>).mock.calls[0][0].exe).toBe("app1.exe");
    expect(document.querySelector(".actions")).toBeNull();
    expect(document.querySelector(".bgpanel")!.innerHTML.length).toBe(before);
  });

  it("mide el tamaño de diseño de la isla, no el escalado por la animación de aparición", async () => {
    const a = api();
    const w = vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(300);
    const h = vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(46);
    const rect = vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({ width: 282, height: 43, x: 0, y: 0, top: 0, left: 0, right: 282, bottom: 43, toJSON() {} });
    try {
      render(TaskbarView, { state: snapshot([]), monitorId: 0, api: a, onWorkspace: () => {} });
      await waitFor(() => expect(a.resize).toHaveBeenCalled());
      expect(a.resize).toHaveBeenCalledWith(302, 48);
    } finally {
      w.mockRestore();
      h.mockRestore();
      rect.mockRestore();
    }
  });

  it("con muchas apps solo hay un botón en la fila (no se ensancha) y la lista vive en su propio panel", () => {
    const many = Array.from({ length: 30 }, (_, i) => bg(i + 1));
    render(TaskbarView, { state: snapshot([]), monitorId: 0, api: api(), onWorkspace: () => {}, background: many });
    expect(document.querySelectorAll(".row .app, .row .card").length).toBe(0);
    expect(screen.getByRole("button", { name: "Aplicaciones en segundo plano (30)" })).toBeTruthy();
  });

  it("sin apps en segundo plano no hay botón, y el de energía solo está si se pasa", () => {
    render(TaskbarView, { state: snapshot([]), monitorId: 0, api: api(), onWorkspace: () => {} });
    expect(screen.queryByRole("button", { name: /segundo plano/ })).toBeNull();
    expect(screen.queryByRole("button", { name: "Energía" })).toBeNull();
    cleanup();
    render(TaskbarView, { state: snapshot([]), monitorId: 0, api: api(), onWorkspace: () => {}, power: { allowChanges: false, run: vi.fn() } });
    expect(screen.getByRole("button", { name: "Energía" })).toBeTruthy();
  });

  it("al abrir Energía se ocultan las apps y solo quedan sus botones y «Volver»; volver restaura la barra", async () => {
    render(TaskbarView, {
      state: snapshot([win(1, 1, "code")]),
      monitorId: 0,
      api: api(),
      onWorkspace: () => {},
      background: [bg(1)],
      power: { allowChanges: true, run: vi.fn() },
    });
    expect(document.querySelectorAll(".group").length).toBe(1);
    await fireEvent.click(screen.getByRole("button", { name: "Energía" }));
    expect(document.querySelectorAll(".group").length).toBe(0);
    expect(screen.queryByRole("button", { name: /segundo plano/ })).toBeNull();
    for (const label of ["Suspender", "Reiniciar", "Hibernar", "Apagar"]) expect(screen.getByText(label)).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Volver a la barra de tareas" }));
    expect(document.querySelectorAll(".group").length).toBe(1);
    expect(screen.getByRole("button", { name: "Energía" })).toBeTruthy();
  });

  it("al cerrarse usa su propia animación de salida", async () => {
    const { rerender } = render(TaskbarView, { state: snapshot([]), monitorId: 0, api: api(), onWorkspace: () => {}, openTick: 0, closeTick: 0 });
    await rerender({ closeTick: 1 });
    expect(document.querySelector(".stage")?.classList.contains("closing")).toBe(true);
  });
});
