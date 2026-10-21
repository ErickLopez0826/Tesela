import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { StateSnapshot } from "../api/state";
import type { BatteryState } from "../stores/battery";
import type { ClaudeActivity, UsageView } from "../api/claude";
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

describe("BarView: batería", () => {
  const withBattery = (battery: BatteryState | null) => render(BarView, { monitorId: 1, state, stats, now, battery, onWorkspace: vi.fn() });

  it("en un equipo sin batería (un escritorio) no muestra nada", () => {
    const { container } = withBattery(null);
    expect(container.querySelector(".bat")).toBeNull();
    expect(container.querySelector(".battery")).toBeNull();
  });

  it("con batería enseña el porcentaje, el modo y el tiempo restante", () => {
    const { container } = withBattery({ percent: 57.4, mode: "on_battery", minutes_left: 121, saver: false });
    const bat = container.querySelector(".bat")!;
    expect(bat.textContent?.trim()).toBe("57%");
    expect(bat.getAttribute("title")).toBe("Batería 57% · con batería, 2 h 1 min restantes");
    expect(container.querySelector(".bolt")).toBeNull();
    expect(bat.classList.contains("low")).toBe(false);
    // La carga se dibuja: 57,4 % de un hueco de 14.
    expect(Number(container.querySelector("[data-fill]")!.getAttribute("width"))).toBeCloseTo(8.04, 1);
  });

  it("cargando lleva un rayo y no se marca como baja aunque tenga poca carga", () => {
    const { container } = withBattery({ percent: 8, mode: "charging", minutes_left: null, saver: false });
    const bat = container.querySelector(".bat")!;
    expect(container.querySelector(".bolt")).not.toBeNull();
    expect(bat.classList.contains("charging")).toBe(true);
    expect(bat.classList.contains("low")).toBe(false);
    expect(bat.getAttribute("title")).toBe("Batería 8% · cargando");
  });

  it("con poca batería y sin cargador se marca como baja", () => {
    const { container } = withBattery({ percent: 15, mode: "on_battery", minutes_left: 20, saver: true });
    expect(container.querySelector(".bat")!.classList.contains("low")).toBe(true);
  });

  it("se cuenta como un dato más, con su separador", () => {
    const { container } = render(BarView, { monitorId: 1, state, stats, now, volume: { percent: 40, muted: false }, battery: { percent: 90, mode: "plugged_in", minutes_left: null, saver: false }, onWorkspace: vi.fn() });
    // CPU, RAM, volumen, batería y hora: cinco datos, cuatro separadores.
    expect(container.querySelectorAll(".dots[aria-hidden='true']").length).toBe(4);
  });
});

describe("BarView: Claude Code", () => {
  it("no dibuja un indicador de estado aparte junto a la hora: la animación está en el anillo del uso", () => {
    for (const activity of ["working", "waiting", "idle"] as const) {
      const { container } = render(BarView, { monitorId: 1, state, stats, now, claude: { activity, sessions: 1 }, onWorkspace: vi.fn() });
      expect(container.querySelector(".claude, .claude-status")).toBeNull();
      cleanup();
    }
  });
});

const NOW_S = Math.floor(Date.now() / 1000);
const usage = (over: Partial<UsageView> = {}): UsageView => ({
  enabled: true,
  status: "ok",
  windows: [
    { id: "five_hour", label: "5 h", percent: 73.4, resets_at: NOW_S + 3060, level: "warn" },
    { id: "seven_day", label: "Semana", percent: 7, resets_at: NOW_S + 3 * 86400, level: "ok" },
  ],
  breakdown: [],
  fetched_at: NOW_S - 60,
  problem: null,
  message: null,
  ...over,
});

describe("BarView: uso de Claude", () => {
  const show = (claudeUsage: UsageView | null, extra = {}) => render(BarView, { monitorId: 1, state, stats, now, claudeUsage, onWorkspace: vi.fn(), ...extra });

  it("sin la función activada, sin lectura o sin ventanas no se ve nada", () => {
    expect(show(null).container.querySelector(".claude-usage")).toBeNull();
    cleanup();
    expect(show(usage({ enabled: false, status: "off", windows: [] })).container.querySelector(".claude-usage")).toBeNull();
    cleanup();
    expect(show(usage({ status: "waiting", windows: [] })).container.querySelector(".claude-usage")).toBeNull();
  });

  it("enseña el anillo de puntos con la marca de Claude y el porcentaje de la sesión de 5 horas", () => {
    const { container } = show(usage());
    const chip = container.querySelector(".claude-usage")!;
    expect(chip.textContent?.trim()).toBe("73%");
    expect(chip.classList.contains("warn")).toBe(true);
    expect(chip.querySelector(".usage-ring .claude-mark")).not.toBeNull();
    // 16 puntos, 73 % → 12 encendidos.
    expect(chip.querySelectorAll(".usage-ring .dot").length).toBe(16);
    expect(chip.querySelectorAll(".usage-ring .dot.on").length).toBe(12);
    expect(chip.getAttribute("aria-label")).toContain("Uso de tu suscripción de Claude");
    expect(chip.getAttribute("aria-label")).toContain("5 h: 73%");
  });

  it("ocupa el sitio de la temperatura: entre la gráfica y el volumen", () => {
    const { container } = show(usage(), { gpu: 12, volume: { percent: 40, muted: false } });
    const order = [...container.querySelectorAll(".zone.right > *:not(.dots)")].map((e) => (e.classList.contains("claude-usage") ? "uso" : e.textContent?.trim().split(" ")[0]));
    expect(order.indexOf("GPU")).toBeLessThan(order.indexOf("uso"));
    expect(order.indexOf("uso")).toBeLessThan(order.findIndex((o) => o === "40%"));
  });

  it("con una lectura vieja se ve atenuado", () => {
    const { container } = show(usage({ status: "stale", problem: "offline", message: "Sin conexión con Claude." }));
    expect(container.querySelector(".claude-usage")!.classList.contains("stale")).toBe(true);
    expect(container.querySelector(".usage-ring")!.classList.contains("dim")).toBe(true);
  });

  it("una sesión ya reiniciada no se enseña como actual: «—» y el anillo apagado", () => {
    const old = usage({ windows: [{ id: "five_hour", label: "5 h", percent: 95, resets_at: NOW_S - 10, level: "danger" }] });
    const { container } = show(old);
    expect(container.querySelector(".claude-usage")!.textContent?.trim()).toBe("—");
    expect(container.querySelectorAll(".usage-ring .dot.on").length).toBe(0);
  });

  it("el anillo se anima solo mientras Claude Code está trabajando", () => {
    const ring = (claude: ClaudeActivity | null) => render(BarView, { monitorId: 1, state, stats, now, claudeUsage: usage(), claude, onWorkspace: vi.fn() }).container.querySelector(".usage-ring")!;
    expect(ring({ activity: "working", sessions: 1 }).classList.contains("working")).toBe(true);
    cleanup();
    expect(ring({ activity: "waiting", sessions: 1 }).classList.contains("working")).toBe(false);
    cleanup();
    expect(ring(null).classList.contains("working")).toBe(false);
  });

  it("al pasar el cursor avisa con el rectángulo del anillo y al salir avisa de que se fue", async () => {
    const onUsageHover = vi.fn();
    const onUsageLeave = vi.fn();
    const { container } = show(usage(), { onUsageHover, onUsageLeave });
    const chip = container.querySelector(".claude-usage") as HTMLElement;
    chip.getBoundingClientRect = () => ({ left: 400, top: 4, width: 50, height: 24, right: 450, bottom: 28, x: 400, y: 4, toJSON() {} }) as DOMRect;
    await fireEvent.mouseEnter(chip);
    expect(onUsageHover).toHaveBeenCalledWith({ x: 400, y: 4, w: 50, h: 24 });
    await fireEvent.mouseLeave(chip);
    expect(onUsageLeave).toHaveBeenCalledTimes(1);
  });
});

describe("BarView: separadores, pastilla y workspaces nuevos", () => {
  it("separa los datos con tres puntos, sin separadores huérfanos", () => {
    const full = render(BarView, { monitorId: 1, state, stats, now, volume: { percent: 40, muted: false }, gpu: 10, claudeUsage: usage(), onWorkspace: vi.fn() });
    // CPU, RAM, GPU, uso de Claude, volumen y hora: seis datos, cinco separadores.
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

describe("BarView: volumen y gráfica", () => {
  it("muestra el volumen y la gráfica, sin la canción ni la temperatura, y silencia con un clic", async () => {
    const onMute = vi.fn();
    render(BarView, { monitorId: 1, state, stats, now, volume: { percent: 40, muted: false }, gpu: 37, onWorkspace: vi.fn(), onMute });
    expect(screen.queryByText("Banda – Tema")).toBeNull();
    expect(screen.getByText(/^GPU 37/)).toBeTruthy();
    // La temperatura de Mérida ya no va en la barra (sigue en el panel «Hoy»): su sitio es del uso de Claude.
    expect(screen.queryByText(/°C/)).toBeNull();
    expect(document.querySelector(".weather")).toBeNull();
    await fireEvent.click(screen.getByLabelText("Silenciar"));
    expect(onMute).toHaveBeenCalledWith(true);
  });
});
