import { describe, expect, it } from "vitest";
import { errorText, hasIcon, letter, moveEntry, moveSelection, usage } from "./search";

describe("moveSelection", () => {
  it("avanza y retrocede dando la vuelta", () => {
    expect(moveSelection(0, 1, 3)).toBe(1);
    expect(moveSelection(2, 1, 3)).toBe(0);
    expect(moveSelection(0, -1, 3)).toBe(2);
  });

  it("sin selección, abajo va al primero y arriba al último", () => {
    expect(moveSelection(-1, 1, 3)).toBe(0);
    expect(moveSelection(-1, -1, 3)).toBe(2);
    expect(moveSelection(7, 1, 3)).toBe(0);
  });

  it("sin resultados no hay selección", () => {
    expect(moveSelection(0, 1, 0)).toBe(-1);
    expect(moveSelection(-1, -1, 0)).toBe(-1);
  });
});

describe("letter", () => {
  it("es la primera letra o cifra en mayúscula", () => {
    expect(letter("spotify")).toBe("S");
    expect(letter("  épico")).toBe("É");
    expect(letter("7-Zip")).toBe("7");
    expect(letter("«Hola»")).toBe("H");
  });

  it("sin letras devuelve un signo", () => {
    expect(letter("")).toBe("?");
    expect(letter("···")).toBe("?");
  });
});

describe("hasIcon", () => {
  it("solo los programas tienen icono propio", () => {
    const k = (kind: "app" | "running" | "redirect" | "web_search") => hasIcon({ id: "1", kind, title: "", subtitle: "" });
    expect([k("app"), k("running"), k("redirect"), k("web_search")]).toEqual([true, true, false, false]);
  });
});

describe("moveEntry", () => {
  it("mueve una posición y no toca la original", () => {
    const list = ["a", "b", "c"];
    expect(moveEntry(list, 1, -1)).toEqual(["b", "a", "c"]);
    expect(moveEntry(list, 1, 1)).toEqual(["a", "c", "b"]);
    expect(list).toEqual(["a", "b", "c"]);
  });

  it("en los extremos o fuera de rango devuelve la misma lista", () => {
    const list = ["a", "b"];
    expect(moveEntry(list, 0, -1)).toBe(list);
    expect(moveEntry(list, 1, 1)).toBe(list);
    expect(moveEntry(list, 5, -1)).toBe(list);
  });
});

describe("usage y errorText", () => {
  it("una búsqueda se escribe con su texto y una redirección sola", () => {
    expect(usage({ word: "g", url: "https://x.com/?q={q}", kind: "search" })).toBe("g texto");
    expect(usage({ word: "claude", url: "https://claude.com", kind: "redirect" })).toBe("claude");
  });

  it("el error es su mensaje o el propio texto", () => {
    expect(errorText("mal")).toBe("mal");
    expect(errorText(new Error("roto"))).toBe("roto");
  });
});
