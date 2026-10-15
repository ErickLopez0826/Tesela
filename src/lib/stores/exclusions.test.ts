import { get } from "svelte/store";
import { describe, expect, it } from "vitest";
import type { ExclusionRule } from "../api/exclusions";
import { createExclusionsStore, isFullyExcluded, normalizeExe } from "./exclusions";

function fakeBackend(initial: ExclusionRule[] = []) {
  let rules = [...initial];
  return {
    list: async () => rules,
    add: async (exe: string, title: string | null) => {
      if (!exe.trim()) throw new Error("el nombre del ejecutable está vacío");
      rules = [...rules, { exe: normalizeExe(exe), title_contains: title }];
      return rules;
    },
    remove: async (exe: string, title: string | null) => {
      rules = rules.filter((r) => !(r.exe === normalizeExe(exe) && r.title_contains === title));
      return rules;
    },
  };
}

describe("normalizeExe", () => {
  it("ignora mayúsculas y .exe", () => {
    expect(normalizeExe(" Overwatch.EXE ")).toBe("overwatch");
  });
});

describe("isFullyExcluded", () => {
  it("el título solo es un criterio adicional: con título no excluye todo el proceso", () => {
    const rules = [{ exe: "chrome", title_contains: "picture in picture" }];
    expect(isFullyExcluded(rules, "chrome.exe")).toBe(false);
    expect(isFullyExcluded([{ exe: "chrome", title_contains: null }], "Chrome.exe")).toBe(true);
  });
});

describe("exclusions store", () => {
  it("carga, agrega y quita", async () => {
    const store = createExclusionsStore(fakeBackend([{ exe: "a", title_contains: null }]));
    await store.refresh();
    expect(get(store).rules).toHaveLength(1);
    await store.add("Juego.exe");
    expect(get(store).rules.map((r) => r.exe)).toEqual(["a", "juego"]);
    await store.remove("juego");
    expect(get(store).rules.map((r) => r.exe)).toEqual(["a"]);
  });

  it("muestra el error si el núcleo rechaza", async () => {
    const store = createExclusionsStore(fakeBackend());
    await store.add("  ");
    expect(get(store).error).toContain("vacío");
    expect(get(store).rules).toEqual([]);
  });
});
