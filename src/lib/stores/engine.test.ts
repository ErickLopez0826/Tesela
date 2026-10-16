import { get } from "svelte/store";
import { describe, expect, it } from "vitest";
import type { EngineWallpaper } from "../api/system";
import { clampPage, favoriteWallpapers, filterWallpapers, pageCount, slicePage, toggleFavorite } from "./engine";

const items: EngineWallpaper[] = [
  { id: "a", title: "Lluvia en Tokio", kind: "scene", preview: null },
  { id: "b", title: "Bosque", kind: "video", preview: null },
  { id: "c", title: "Panel web", kind: "web", preview: null },
];
const base = { query: "", kind: "all" as const, onlyFavorites: false, favorites: new Set<string>() };

describe("fondos de Wallpaper Engine", () => {
  it("busca por título sin distinguir mayúsculas ni acentos", () => {
    expect(filterWallpapers(items, { ...base, query: "tokio lluvia" }).map((w) => w.id)).toEqual(["a"]);
    expect(filterWallpapers(items, { ...base, query: "BOSQUE" }).map((w) => w.id)).toEqual(["b"]);
    expect(filterWallpapers(items, { ...base, query: "inexistente" })).toEqual([]);
  });

  it("filtra por tipo y por guardados", () => {
    expect(filterWallpapers(items, { ...base, kind: "video" }).map((w) => w.id)).toEqual(["b"]);
    expect(filterWallpapers(items, { ...base, onlyFavorites: true, favorites: new Set(["c"]) }).map((w) => w.id)).toEqual(["c"]);
  });

  it("pagina: número de páginas, límites y contenido de cada una", () => {
    expect(pageCount(0)).toBe(1);
    expect(pageCount(18)).toBe(1);
    expect(pageCount(19)).toBe(2);
    expect(pageCount(263)).toBe(15);
    expect(clampPage(-3, 40)).toBe(0);
    expect(clampPage(9, 40)).toBe(2);
    const all = Array.from({ length: 40 }, (_, i) => i);
    expect(slicePage(all, 0)).toEqual(all.slice(0, 18));
    expect(slicePage(all, 2)).toEqual(all.slice(36, 40));
    expect(slicePage(all, 99)).toEqual(all.slice(36, 40));
    expect(slicePage([], 0)).toEqual([]);
  });

  it("marcar y desmarcar un favorito", () => {
    toggleFavorite("a");
    expect(get(favoriteWallpapers).has("a")).toBe(true);
    toggleFavorite("a");
    expect(get(favoriteWallpapers).has("a")).toBe(false);
  });
});
