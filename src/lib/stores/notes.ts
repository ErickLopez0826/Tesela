import type { Note } from "../api/system";

/** Sin acentos y en minúsculas, para que «cafe» encuentre «Café». */
function fold(s: string): string {
  return s.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
}

/** Notas que contienen todas las palabras de la búsqueda (sin distinguir mayúsculas ni acentos). */
export function filterNotes(notes: Note[], query: string): Note[] {
  const words = fold(query).split(/\s+/).filter(Boolean);
  if (words.length === 0) return notes;
  return notes.filter((n) => {
    const text = fold(n.text);
    return words.every((w) => text.includes(w));
  });
}

const pad = (n: number) => String(n).padStart(2, "0");

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

/** Primera línea de la nota, recortada, para el título de la tarjeta. */
export function headline(text: string, max = 70): string {
  const first = text.split("\n").find((l) => l.trim() !== "") ?? "";
  const t = first.trim();
  return t.length > max ? t.slice(0, max - 1) + "…" : t;
}
