import { describe, expect, it } from "vitest";
import { signalText, wifiArcs } from "./wifi";

describe("wifiArcs", () => {
  it("la señal se reparte entre los tres arcos", () => {
    expect([0, 1, 2, 3, 4, 5].map(wifiArcs)).toEqual([0, 1, 1, 2, 2, 3]);
  });

  it("cualquier señal enciende al menos un arco, y más de 5 barras no se pasa de tres", () => {
    expect(wifiArcs(1)).toBe(1);
    expect(wifiArcs(9)).toBe(3);
  });

  it("sin conexión solo queda el punto, y si no se sabe el icono se ve completo", () => {
    expect(wifiArcs(null)).toBe(0);
    expect(wifiArcs(-1)).toBe(0);
    expect(wifiArcs(undefined)).toBe(3);
  });
});

describe("signalText", () => {
  it("dice la señal, o que no hay conexión, o nada si no se sabe", () => {
    expect(signalText(4)).toBe("Señal: 4 de 5");
    expect(signalText(9)).toBe("Señal: 5 de 5");
    expect(signalText(null)).toBe("Sin conexión");
    expect(signalText(undefined)).toBe("");
  });
});
