import type { Note } from "../api/system";

/** Sin acentos y en minúsculas, para que «cafe» encuentre «Café». */
function fold(s: string): string {
  return s.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
}

const pad = (n: number) => String(n).padStart(2, "0");

/** «2026-10-16»: el día de una nota en hora local (es lo que comparan los filtros de fecha). */
export function localDay(ms: number): string {
  const d = new Date(ms);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** Rango de fechas para buscar: días «AAAA-MM-DD» (los que da un campo de fecha), los dos incluidos. Vacío = sin límite. */
export interface DateRange {
  from?: string;
  to?: string;
}

export type RangePreset = "today" | "yesterday" | "week" | "month";

export const RANGE_PRESETS: { id: RangePreset; label: string }[] = [
  { id: "today", label: "Hoy" },
  { id: "yesterday", label: "Ayer" },
  { id: "week", label: "7 días" },
  { id: "month", label: "Este mes" },
];

/** El rango de un atajo: «7 días» incluye hoy y los seis anteriores; «Este mes» va del día 1 a hoy. */
export function presetRange(id: RangePreset, now: Date = new Date()): Required<DateRange> {
  const at = (back: number) => localDay(new Date(now.getFullYear(), now.getMonth(), now.getDate() - back, 12).getTime());
  const today = at(0);
  switch (id) {
    case "today":
      return { from: today, to: today };
    case "yesterday":
      return { from: at(1), to: at(1) };
    case "week":
      return { from: at(6), to: today };
    case "month":
      return { from: localDay(new Date(now.getFullYear(), now.getMonth(), 1, 12).getTime()), to: today };
  }
}

/** «16/10/2026» a partir de «2026-10-16». */
export function showDay(day: string): string {
  const [y, m, d] = day.split("-");
  return y && m && d ? `${d}/${m}/${y}` : day;
}

/** Resumen del rango para mostrarlo: «Del 10/10/2026 al 16/10/2026», «Desde …», «Hasta …» o «El 10/10/2026». */
export function describeRange({ from, to }: DateRange): string {
  if (from && to) return from === to ? `El ${showDay(from)}` : `Del ${showDay(from)} al ${showDay(to)}`;
  if (from) return `Desde el ${showDay(from)}`;
  if (to) return `Hasta el ${showDay(to)}`;
  return "";
}

/**
 * Notas que contienen todas las palabras de la búsqueda (sin distinguir mayúsculas ni acentos) y cuyo día está dentro del
 * rango. Las palabras también se buscan en la fecha de la nota: «16/10», «10/2026» o «2026-10-16» encuentran las de ese día.
 */
export function filterNotes(notes: Note[], query: string, range: DateRange = {}): Note[] {
  const words = fold(query).split(/\s+/).filter(Boolean);
  const { from, to } = range;
  if (words.length === 0 && !from && !to) return notes;
  return notes.filter((n) => {
    const day = localDay(n.created);
    if ((from && day < from) || (to && day > to)) return false;
    const haystack = `${fold(n.text)}\n${formatStamp(n.created)}\n${day}`;
    return words.every((w) => haystack.includes(w));
  });
}

/** «16/10/2026 14:05» en hora local. */
export function formatStamp(ms: number): string {
  const d = new Date(ms);
  return `${pad(d.getDate())}/${pad(d.getMonth() + 1)}/${d.getFullYear()} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** Cada nota bajo un encabezado con su fecha; separadas por una línea. */
export function toMarkdown(notes: Note[]): string {
  const body = notes.map((n) => `## ${formatStamp(n.created)}\n\n${n.text}\n`).join("\n---\n\n");
  return `# Notas\n\n${body}`;
}

/** Texto plano: la fecha entre corchetes y la nota, separadas por una línea de guiones. */
export function toPlainText(notes: Note[]): string {
  return notes.map((n) => `[${formatStamp(n.created)}]\n${n.text}\n`).join("\n----------------------------------------\n\n");
}

export type ExportFormat = "md" | "txt";

export function exportContent(notes: Note[], format: ExportFormat): string {
  return format === "md" ? toMarkdown(notes) : toPlainText(notes);
}

/** «notas-2026-10-16.md» (la fecha es la de hoy, en hora local). */
export function exportName(format: ExportFormat, now: Date = new Date()): string {
  return `notas-${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}.${format}`;
}

/** Nombre del archivo según lo que se exporta: «nota-2026-10-16-1405.md» si es una sola y «notas-2026-10-16.md» si son varias. */
export function exportFileName(notes: Note[], format: ExportFormat, now: Date = new Date()): string {
  if (notes.length !== 1) return exportName(format, now);
  const d = new Date(notes[0].created);
  return `nota-${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}-${pad(d.getHours())}${pad(d.getMinutes())}.${format}`;
}

/** Primera línea de la nota, recortada, para el título de la tarjeta. */
export function headline(text: string, max = 70): string {
  const first = text.split("\n").find((l) => l.trim() !== "") ?? "";
  const t = first.trim();
  return t.length > max ? t.slice(0, max - 1) + "…" : t;
}
