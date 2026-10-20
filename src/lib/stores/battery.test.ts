import { describe, expect, it } from "vitest";
import { describeBattery, formatMinutes, isLow, type BatteryState } from "./battery";

const battery = (over: Partial<BatteryState> = {}): BatteryState => ({ percent: 57, mode: "on_battery", minutes_left: null, saver: false, ...over });

describe("batería", () => {
  it("da el tiempo restante en horas y minutos", () => {
    expect(formatMinutes(45)).toBe("45 min");
    expect(formatMinutes(60)).toBe("1 h");
    expect(formatMinutes(125)).toBe("2 h 5 min");
  });

  it("describe cada modo", () => {
    expect(describeBattery(battery({ mode: "charging", percent: 64 }))).toBe("Batería 64% · cargando");
    expect(describeBattery(battery({ mode: "charging", percent: 100 }))).toBe("Batería 100% · carga completa");
    expect(describeBattery(battery({ mode: "plugged_in", percent: 80 }))).toBe("Batería 80% · conectada, sin cargar");
    expect(describeBattery(battery({ mode: "plugged_in", percent: 100 }))).toBe("Batería 100% · carga completa, conectada");
    expect(describeBattery(battery())).toBe("Batería 57% · con batería");
    expect(describeBattery(battery({ minutes_left: 121, saver: true }))).toBe("Batería 57% · con batería, 2 h 1 min restantes · ahorro de batería");
  });

  it("solo es baja sin cargador y con 20 % o menos", () => {
    expect(isLow(battery({ percent: 20 }))).toBe(true);
    expect(isLow(battery({ percent: 21 }))).toBe(false);
    expect(isLow(battery({ percent: 5, mode: "charging" }))).toBe(false);
    expect(isLow(battery({ percent: 5, mode: "plugged_in" }))).toBe(false);
  });
});
