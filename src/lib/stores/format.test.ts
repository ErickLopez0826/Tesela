import { describe, expect, it } from "vitest";
import { formatBytes, formatRate, formatUptime, percentOf } from "./format";

describe("formatBytes", () => {
  it("elige la unidad", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(5 * 1024 ** 3)).toBe("5.0 GB");
    expect(formatBytes(2 * 1024 ** 4)).toBe("2.0 TB");
    expect(formatBytes(-5)).toBe("0 B");
  });

  it("velocidad", () => {
    expect(formatRate(2 * 1024 * 1024)).toBe("2.0 MB/s");
  });
});

describe("formatUptime", () => {
  it("minutos, horas y días", () => {
    expect(formatUptime(90)).toBe("1 min");
    expect(formatUptime(2 * 3600 + 37 * 60)).toBe("2 h 37 min");
    expect(formatUptime(3 * 86400 + 4 * 3600 + 5)).toBe("3 d 4 h");
  });
});

describe("percentOf", () => {
  it("acota y evita dividir por cero", () => {
    expect(percentOf(1, 4)).toBe(25);
    expect(percentOf(5, 4)).toBe(100);
    expect(percentOf(1, 0)).toBe(0);
  });
});
