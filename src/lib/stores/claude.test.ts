import { describe, expect, it } from "vitest";
import type { UsageView, UsageWindow } from "../api/claude";
import { describeUsage, formatAge, formatRemaining, headline, isOutdated, resetLabel, usageLevel, usageText, windowTitle } from "./claude";

const NOW = 1_000_000;
const win = (id: string, percent: number, resetsIn: number | null, level: UsageWindow["level"] = "ok"): UsageWindow => ({
  id,
  label: id === "five_hour" ? "5 h" : "Semana",
  percent,
  resets_at: resetsIn === null ? null : NOW + resetsIn,
  level,
});
const view = (windows: UsageWindow[], over: Partial<UsageView> = {}): UsageView => ({ enabled: true, status: "ok", windows, breakdown: [], fetched_at: NOW - 180, problem: null, message: null, ...over });

describe("tiempos", () => {
  it("formatRemaining da horas y minutos, días y horas, o nada si ya pasó", () => {
    expect(formatRemaining(NOW + 30, NOW)).toBe("menos de 1 min");
    expect(formatRemaining(NOW + 45 * 60, NOW)).toBe("45 min");
    expect(formatRemaining(NOW + 8100, NOW)).toBe("2 h 15 min");
    expect(formatRemaining(NOW + 7200, NOW)).toBe("2 h");
    expect(formatRemaining(NOW + 4 * 86400 + 3 * 3600, NOW)).toBe("4 d 3 h");
    expect(formatRemaining(NOW + 2 * 86400, NOW)).toBe("2 d");
    expect(formatRemaining(NOW, NOW)).toBeNull();
    expect(formatRemaining(NOW - 5, NOW)).toBeNull();
  });

  it("formatAge dice hace cuánto", () => {
    expect(formatAge(NOW - 10, NOW)).toBe("hace un momento");
    expect(formatAge(NOW - 180, NOW)).toBe("hace 3 min");
    expect(formatAge(NOW - 2 * 3600, NOW)).toBe("hace 2 h");
    expect(formatAge(NOW - 3 * 86400, NOW)).toBe("hace 3 d");
    expect(formatAge(NOW + 100, NOW)).toBe("hace un momento");
  });
});

describe("uso de la suscripción", () => {
  const five = win("five_hour", 35, 8100);
  const week = win("seven_day", 24, 4 * 86400);

  it("el resumen son las ventanas de 5 horas y semana, con su porcentaje", () => {
    expect(usageText(view([five, week, win("seven_day_opus", 90, 100, "danger")]), NOW)).toBe("5 h 35% · Sem 24%");
    expect(headline(view([week])).length).toBe(1);
  });

  it("una ventana ya reiniciada no se enseña como actual", () => {
    const old = win("five_hour", 95, -10, "danger");
    expect(isOutdated(old, NOW)).toBe(true);
    expect(isOutdated(win("five_hour", 5, null), NOW)).toBe(false);
    expect(usageText(view([old, week]), NOW)).toBe("5 h — · Sem 24%");
    // Y no cuenta para el color.
    expect(usageLevel(view([old, week]), NOW)).toBe("ok");
  });

  it("el nivel es el peor de las ventanas vigentes", () => {
    expect(usageLevel(view([five, week]), NOW)).toBe("ok");
    expect(usageLevel(view([five, win("seven_day", 75, 100, "warn")]), NOW)).toBe("warn");
    expect(usageLevel(view([win("five_hour", 95, 100, "danger"), win("seven_day", 75, 100, "warn")]), NOW)).toBe("danger");
  });

  it("el detalle lleva reinicios, desglose, antigüedad y, si es viejo, el motivo", () => {
    const text = describeUsage(view([five, week], { breakdown: [{ name: "Claude Code", percent: 88 }, { name: "Chats", percent: 1 }] }), NOW);
    expect(text).toBe("5 h: 35% · reinicia en 2 h 15 min\nSemana: 24% · reinicia en 4 d\nEsta semana: Claude Code 88% · Chats 1%\nLeído hace 3 min");
    const stale = describeUsage(view([five], { status: "stale", message: "Sin conexión con Claude." }), NOW);
    expect(stale).toContain("Dato viejo: Sin conexión con Claude.");
    expect(describeUsage(view([win("five_hour", 95, -10, "danger")]), NOW)).toContain("5 h: ya se reinició");
  });
});

describe("tarjeta", () => {
  it("cada ventana tiene su nombre largo", () => {
    expect(windowTitle(win("five_hour", 1, 1))).toBe("Sesión actual");
    expect(windowTitle(win("seven_day", 1, 1))).toBe("Todos los modelos");
    expect(windowTitle({ ...win("seven_day_opus", 1, 1), label: "Semana Opus" })).toBe("Opus");
    expect(windowTitle({ ...win("otra", 1, 1), label: "Rara" })).toBe("Rara");
  });

  it("el reinicio cercano va «en X» y el lejano con día y hora", () => {
    expect(resetLabel(win("five_hour", 1, 51 * 60 + 30), NOW)).toBe("Reinicia en 51 min");
    expect(resetLabel(win("five_hour", 1, 86_399), NOW)).toMatch(/^Reinicia en 23 h 59 min$/);
    expect(resetLabel(win("seven_day", 1, 3 * 86_400), NOW)).toMatch(/^Reinicia [a-záéíóú]{3} \d{1,2}:\d{2}/);
    expect(resetLabel(win("seven_day", 1, -1), NOW)).toBe("Reiniciada");
    expect(resetLabel(win("seven_day", 1, null), NOW)).toBe("");
  });
});
