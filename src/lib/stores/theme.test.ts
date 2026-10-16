import { describe, expect, it } from "vitest";
import { accentFromWallpaper, cssVars, DEFAULT_THEME, onColor, parseHex, sanitize, toHex } from "./theme";

describe("tema", () => {
  it("lee y escribe colores hexadecimales", () => {
    expect(parseHex("#FF8000")).toEqual([255, 128, 0]);
    expect(parseHex("rojo")).toBeNull();
    expect(toHex([255, 128, 0])).toBe("#ff8000");
  });

  it("el texto sobre un color es negro si es claro y blanco si es oscuro", () => {
    expect(onColor("#f2f2f2")).toBe("#0a0a0b");
    expect(onColor("#102040")).toBe("#ffffff");
  });

  it("sanea lo guardado: colores inválidos vuelven al predeterminado", () => {
    const t = sanitize({ mode: "wallpaper", accent: "nada", dots: "#00FF00" });
    expect(t.mode).toBe("wallpaper");
    expect(t.accent).toBe(DEFAULT_THEME.accent);
    expect(t.dots).toBe("#00ff00");
    expect(sanitize(null)).toEqual(DEFAULT_THEME);
  });

  it("con colores propios cada elemento usa el suyo", () => {
    const v = cssVars({ ...DEFAULT_THEME, accent: "#ff0000", dots: "#00ff00", frame: "#0000ff", workspace: "#ffff00" }, "#123456");
    expect(v["--accent"]).toBe("#ff0000");
    expect(v["--dot"]).toBe("0, 255, 0");
    expect(v["--frame"]).toBe("0, 0, 255");
    expect(v["--ws"]).toBe("#ffff00");
  });

  it("en modo fondo todo sale del color del fondo", () => {
    const v = cssVars({ ...DEFAULT_THEME, mode: "wallpaper" }, "#2060c0");
    const accent = accentFromWallpaper("#2060c0");
    expect(v["--accent"]).toBe(accent);
    expect(v["--ws"]).toBe(accent);
    expect(v["--dot"]).toBe(v["--frame"]);
    // Sin color de fondo disponible se usan los colores propios.
    expect(cssVars({ ...DEFAULT_THEME, mode: "wallpaper" }, null)["--accent"]).toBe(DEFAULT_THEME.accent);
  });

  it("el acento del fondo siempre es claro", () => {
    const [r, g, b] = parseHex(accentFromWallpaper("#191717"))!;
    expect((r + g + b) / 3).toBeGreaterThan(150);
  });

  it("un fondo oscuro y apagado da un acento con color, no un gris", () => {
    const [r, g, b] = parseHex(accentFromWallpaper("#60483f"))!;
    expect(Math.max(r, g, b) - Math.min(r, g, b)).toBeGreaterThan(40);
    const [r2, g2, b2] = parseHex(accentFromWallpaper("#141216"))!;
    expect(Math.max(r2, g2, b2) - Math.min(r2, g2, b2)).toBeGreaterThan(40);
  });

  it("solo un fondo sin nada de color da un acento gris", () => {
    const [r, g, b] = parseHex(accentFromWallpaper("#808080"))!;
    expect(Math.max(r, g, b) - Math.min(r, g, b)).toBeLessThan(4);
  });
});
