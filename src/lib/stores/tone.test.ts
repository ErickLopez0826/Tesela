import { describe, expect, it } from "vitest";
import { LIGHT_BACKGROUND, TINT_LUMA, effectiveLuma, imageLuma, toneFor } from "./tone";

describe("color de texto dinámico", () => {
  it("fondo oscuro: texto claro; fondo claro: texto oscuro", () => {
    expect(toneFor(0.1).fg).toBe("#f1f1f1");
    expect(toneFor(LIGHT_BACKGROUND).fg).toBe("#f1f1f1");
    expect(toneFor(0.9).fg).toBe("#111114");
    expect(toneFor(0.9).muted).not.toBe(toneFor(0.1).muted);
  });

  it("con el tinte opaco no importa lo que haya detrás", () => {
    expect(effectiveLuma(1, 1)).toBeCloseTo(TINT_LUMA);
    expect(toneFor(effectiveLuma(1, 1)).fg).toBe("#f1f1f1");
  });

  it("con el tinte muy transparente manda el fondo", () => {
    expect(effectiveLuma(0.9, 0.1)).toBeGreaterThan(LIGHT_BACKGROUND);
    expect(toneFor(effectiveLuma(0.9, 0.1)).fg).toBe("#111114");
    expect(toneFor(effectiveLuma(0.1, 0.1)).fg).toBe("#f1f1f1");
  });

  it("a medias se mezclan", () => {
    expect(effectiveLuma(1, 0.5)).toBeCloseTo((TINT_LUMA + 1) / 2);
  });

  it("sin dato del fondo se supone oscuro, y la opacidad se limita a 0..1", () => {
    expect(toneFor(effectiveLuma(null, 0.2)).fg).toBe("#f1f1f1");
    expect(effectiveLuma(0.8, 5)).toBeCloseTo(TINT_LUMA);
    expect(effectiveLuma(0.8, -3)).toBeCloseTo(0.8);
  });

  it("una imagen que no se puede leer da null", async () => {
    expect(await imageLuma(null)).toBeNull();
    expect(await imageLuma("")).toBeNull();
  });
});
