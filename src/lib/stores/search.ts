import type { Redirect, RedirectKind, SearchItem } from "../api/search";

/** Cuántos resultados caben a la vista antes de desplazar. */
export const VISIBLE_RESULTS = 8;

/** Mueve la selección con las flechas, dando la vuelta en los extremos. Sin resultados no hay selección (-1). */
export function moveSelection(current: number, delta: number, length: number): number {
  if (length <= 0) return -1;
  if (current < 0 || current >= length) return delta < 0 ? length - 1 : 0;
  return (current + delta + length) % length;
}

/** Primera letra para el icono de respaldo, en mayúscula. */
export function letter(title: string): string {
  const first = title.trim().match(/[\p{L}\p{N}]/u);
  return first ? first[0].toUpperCase() : "?";
}

/** ¿El resultado es un programa (tiene icono propio que pedir)? */
export function hasIcon(item: SearchItem): boolean {
  return item.kind === "app" || item.kind === "running";
}

/** Nombre de cada tipo de entrada de Ajustes. */
export const REDIRECT_KIND_LABEL: Record<RedirectKind, string> = { redirect: "Redirección", search: "Búsqueda web" };

/** Ejemplo de dirección para cada tipo, en el campo de Ajustes. */
export const URL_PLACEHOLDER: Record<RedirectKind, string> = {
  redirect: "https://claude.com",
  search: "https://www.google.com/search?q={q}",
};

/** Lo que se escribe para probar una entrada, para enseñarlo en la lista: «claude» o «g texto». */
export function usage(r: Redirect): string {
  return r.kind === "search" ? `${r.word} texto` : r.word;
}

/** Mueve un elemento una posición arriba (-1) o abajo (1); si no se puede, devuelve la misma lista. */
export function moveEntry<T>(list: T[], index: number, delta: -1 | 1): T[] {
  const to = index + delta;
  if (index < 0 || index >= list.length || to < 0 || to >= list.length) return list;
  const next = [...list];
  [next[index], next[to]] = [next[to], next[index]];
  return next;
}

/** Texto de un error del núcleo, sin el envoltorio de Tauri. */
export function errorText(e: unknown): string {
  return String((e as { message?: string })?.message ?? e);
}
