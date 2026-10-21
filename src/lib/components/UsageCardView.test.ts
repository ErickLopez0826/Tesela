import { cleanup, render, waitFor } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ClaudeActivity, UsageView } from "../api/claude";
import UsageCardView from "./UsageCardView.svelte";
import UsageRing from "./UsageRing.svelte";

afterEach(cleanup);

const NOW = Math.floor(Date.now() / 1000);
const usage = (over: Partial<UsageView> = {}): UsageView => ({
  enabled: true,
  status: "ok",
  windows: [
    { id: "five_hour", label: "5 h", percent: 73, resets_at: NOW + 51 * 60 + 30, level: "warn" },
    { id: "seven_day", label: "Semana", percent: 7, resets_at: NOW + 3 * 86400, level: "ok" },
  ],
  breakdown: [{ name: "Claude Code", percent: 88 }, { name: "Chats", percent: 1 }],
  fetched_at: NOW - 180,
  problem: null,
  message: null,
  ...over,
});

describe("UsageCardView", () => {
  it("al abrirse lee el uso y enseña cada ventana con su reinicio, su barra de puntos y su porcentaje", async () => {
    const load = vi.fn().mockResolvedValue(usage());
    const { container } = render(UsageCardView, { load, openTick: 1 });
    await waitFor(() => expect(container.querySelectorAll(".win").length).toBe(2));
    expect(load).toHaveBeenCalledTimes(1);
    const [session, week] = [...container.querySelectorAll(".win")];
    expect(session.querySelector(".name")?.textContent).toBe("Sesión actual");
    expect(session.querySelector(".reset")?.textContent).toBe("Reinicia en 51 min");
    expect(session.querySelector(".used")?.textContent).toBe("73% usado");
    // 28 puntos, 73 % → 20 encendidos; color de aviso.
    expect(session.querySelectorAll(".bar i").length).toBe(28);
    expect(session.querySelectorAll(".bar i.on").length).toBe(20);
    expect(session.classList.contains("warn")).toBe(true);
    expect(week.querySelector(".name")?.textContent).toBe("Todos los modelos");
    // Falta más de un día: día y hora en vez de «en X».
    expect(week.querySelector(".reset")?.textContent).toMatch(/^Reinicia [a-záéíóú]{3} \d{1,2}:\d{2}/);
    expect(week.querySelectorAll(".bar i.on").length).toBe(2);
    expect(container.querySelector(".share")?.textContent).toBe("Esta semana · Claude Code 88% · Chats 1%");
    expect(container.querySelector(".foot")?.textContent).toContain("Leído hace 3 min");
    expect(container.querySelector(".mark .claude-mark")).not.toBeNull();
  });

  it("cada vez que se abre vuelve a leer, y al cerrarse reproduce la animación al revés", async () => {
    const load = vi.fn().mockResolvedValue(usage());
    const { container, rerender } = render(UsageCardView, { load, openTick: 1, closeTick: 0 });
    await waitFor(() => expect(load).toHaveBeenCalledTimes(1));
    await rerender({ load, openTick: 1, closeTick: 1 });
    expect(container.querySelector(".stage")!.classList.contains("closing")).toBe(true);
    await rerender({ load, openTick: 2, closeTick: 1 });
    await waitFor(() => expect(load).toHaveBeenCalledTimes(2));
    expect(container.querySelector(".stage")!.classList.contains("closing")).toBe(false);
  });

  it("una ventana ya reiniciada se enseña como tal, sin porcentaje", async () => {
    const old = usage({ windows: [{ id: "five_hour", label: "5 h", percent: 95, resets_at: NOW - 5, level: "danger" }] });
    const { container } = render(UsageCardView, { load: vi.fn().mockResolvedValue(old), openTick: 1 });
    await waitFor(() => expect(container.querySelector(".win")).not.toBeNull());
    expect(container.querySelector(".reset")?.textContent).toBe("Reiniciada");
    expect(container.querySelector(".used")?.textContent).toBe("—");
    expect(container.querySelectorAll(".bar i.on").length).toBe(0);
  });

  it("con una lectura vieja añade el motivo, y sin datos lo dice", async () => {
    const stale = usage({ status: "stale", problem: "offline", message: "Sin conexión con Claude." });
    const a = render(UsageCardView, { load: vi.fn().mockResolvedValue(stale), openTick: 1 });
    await waitFor(() => expect(a.container.querySelector(".foot")).not.toBeNull());
    expect(a.container.querySelector(".foot")!.textContent).toContain("Sin conexión con Claude.");
    cleanup();
    const b = render(UsageCardView, { load: vi.fn().mockResolvedValue(usage({ windows: [], status: "waiting" })), openTick: 1 });
    await waitFor(() => expect(b.container.querySelector(".empty")?.textContent).toBe("Sin datos todavía."));
  });

  it("enseña lo que Claude tiene en marcha: sesiones con su origen y subagentes, y los que esperan", async () => {
    const activity: ClaudeActivity = {
      activity: "working",
      sessions: 2,
      agents: [
        { name: "Tesela", source: "Claude Code · VS Code", state: "working", sub: false },
        { name: "Explore", source: "Subagente", state: "working", sub: true },
        { name: "Web", source: "Claude Code · terminal", state: "waiting", sub: false },
      ],
    };
    const { container } = render(UsageCardView, { load: vi.fn().mockResolvedValue(usage()), loadActivity: vi.fn().mockResolvedValue(activity), openTick: 1 });
    await waitFor(() => expect(container.querySelectorAll(".agent").length).toBe(3));
    const rows = [...container.querySelectorAll(".agent")];
    expect(rows[0].querySelector(".aname")?.textContent).toBe("Tesela");
    expect(rows[0].querySelector(".asrc")?.textContent).toBe("Claude Code · VS Code");
    expect(rows[1].classList.contains("sub")).toBe(true);
    expect(rows[1].querySelector(".asrc")?.textContent).toBe("Subagente");
    expect(rows[2].classList.contains("waiting")).toBe(true);
    expect(rows[2].querySelector(".asrc")?.textContent).toBe("te espera · Claude Code · terminal");
  });

  it("sin nada en marcha lo dice, y se vuelve a leer mientras está abierta", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const loadActivity = vi.fn().mockResolvedValue({ activity: "idle", sessions: 0, agents: [] });
      const { container } = render(UsageCardView, { load: vi.fn().mockResolvedValue(usage()), loadActivity, openTick: 1 });
      await waitFor(() => expect(container.querySelector(".none")?.textContent).toBe("Nada trabajando ahora."));
      const first = loadActivity.mock.calls.length;
      await vi.advanceTimersByTimeAsync(2100);
      expect(loadActivity.mock.calls.length).toBeGreaterThan(first);
    } finally {
      vi.useRealTimers();
    }
  });

  it("sigue la personalización: cristal con tinte, puntos del fondo y texto que se adapta a lo que hay detrás", async () => {
    const { container } = render(UsageCardView, { load: vi.fn().mockResolvedValue(usage()), openTick: 1 });
    const box = container.querySelector(".box") as HTMLElement;
    expect(container.querySelector(".box > .tint")).not.toBeNull();
    expect(container.querySelector(".box .dots")).not.toBeNull();
    // Sin lectura de lo que hay detrás se supone un fondo oscuro: texto claro.
    expect(box.style.getPropertyValue("--fg")).toBe("#f1f1f1");
    expect(box.style.getPropertyValue("--muted")).toBe("#8b8b90");
  });

  it("pide al núcleo el tamaño que necesita su contenido", async () => {
    const resize = vi.fn();
    render(UsageCardView, { load: vi.fn().mockResolvedValue(usage()), resize, openTick: 1 });
    await waitFor(() => expect(resize).toHaveBeenCalled());
  });
});

describe("UsageRing", () => {
  const lit = (c: HTMLElement) => c.querySelectorAll(".dot.on").length;

  it("enciende los puntos que corresponden al porcentaje, con al menos uno si se ha gastado algo", () => {
    for (const [percent, expected] of [[0, 0], [1, 1], [50, 8], [100, 16], [150, 16], [null, 0]] as const) {
      const { container } = render(UsageRing, { percent });
      expect(lit(container), String(percent)).toBe(expected);
      cleanup();
    }
  });

  it("lleva el nivel como clase y se atenúa si el dato es viejo", () => {
    const { container } = render(UsageRing, { percent: 95, level: "danger", dim: true });
    const ring = container.querySelector(".usage-ring")!;
    expect(ring.classList.contains("danger")).toBe(true);
    expect(ring.classList.contains("dim")).toBe(true);
  });

  it("mientras Claude trabaja el anillo lo muestra con una clase para animarse", () => {
    expect(render(UsageRing, { percent: 10, working: true }).container.querySelector(".usage-ring")!.classList.contains("working")).toBe(true);
    cleanup();
    expect(render(UsageRing, { percent: 10 }).container.querySelector(".usage-ring")!.classList.contains("working")).toBe(false);
  });
});
