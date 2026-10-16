import { writable, get } from "svelte/store";
import type { EngineWallpaper } from "../api/system";

export type KindFilter = "all" | "scene" | "video" | "web";

export const KIND_FILTERS: { id: KindFilter; label: string }[] = [
  { id: "all", label: "Todos" },
  { id: "scene", label: "Escena" },
  { id: "video", label: "Video" },
  { id: "web", label: "Web" },
];

function fold(s: string): string {
  return s.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
}

export interface EngineFilter {
  query: string;
  kind: KindFilter;
  onlyFavorites: boolean;
  favorites: ReadonlySet<string>;
}

/** Fondos que cumplen la búsqueda (todas las palabras en el título), el tipo y, si se pide, solo los guardados. */
export function filterWallpapers(items: EngineWallpaper[], f: EngineFilter): EngineWallpaper[] {
  const words = fold(f.query).split(/\s+/).filter(Boolean);
  return items.filter((w) => {
    if (f.onlyFavorites && !f.favorites.has(w.id)) return false;
    if (f.kind !== "all" && w.kind !== f.kind) return false;
    const title = fold(w.title);
    return words.every((word) => title.includes(word));
  });
}

/** Fondos por página: dos filas de seis. */
export const PAGE_SIZE = 18;

export function pageCount(total: number, size: number = PAGE_SIZE): number {
  return Math.max(1, Math.ceil(total / size));
}

/** Página válida (de 0 en adelante) más cercana a `page`. */
export function clampPage(page: number, total: number, size: number = PAGE_SIZE): number {
  return Math.min(Math.max(0, page), pageCount(total, size) - 1);
}

export function slicePage<T>(items: T[], page: number, size: number = PAGE_SIZE): T[] {
  const p = clampPage(page, items.length, size);
  return items.slice(p * size, p * size + size);
}

const KEY = "wm-favorite-wallpapers";

function load(): Set<string> {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    return new Set(Array.isArray(raw) ? raw.filter((x): x is string => typeof x === "string") : []);
  } catch {
    return new Set();
  }
}

/** Fondos guardados como favoritos (se recuerdan entre sesiones y se comparten entre ventanas). */
export const favoriteWallpapers = writable<Set<string>>(load());

export function toggleFavorite(id: string) {
  const next = new Set(get(favoriteWallpapers));
  if (!next.delete(id)) next.add(id);
  try {
    localStorage.setItem(KEY, JSON.stringify([...next]));
  } catch {
    /* sin almacenamiento: vale solo para esta sesión */
  }
  favoriteWallpapers.set(next);
}
