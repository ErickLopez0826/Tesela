import { describe, expect, it } from "vitest";
import type { Note } from "../api/system";
import { exportContent, exportName, filterNotes, formatStamp, headline, toMarkdown, toPlainText } from "./notes";

const at = (y: number, mo: number, d: number, h = 9, mi = 5) => new Date(y, mo - 1, d, h, mi).getTime();
const notes: Note[] = [
  { id: "1", text: "Comprar café y pan", created: at(2026, 10, 16), updated: at(2026, 10, 16) },
  { id: "2", text: "Llamar a Ana\nsobre el proyecto", created: at(2026, 10, 15, 18, 30), updated: at(2026, 10, 15, 18, 30) },
];

describe("notas rápidas", () => {
  it("la búsqueda ignora mayúsculas y acentos y pide todas las palabras", () => {
    expect(filterNotes(notes, "CAFE").map((n) => n.id)).toEqual(["1"]);
    expect(filterNotes(notes, "ana proyecto").map((n) => n.id)).toEqual(["2"]);
    expect(filterNotes(notes, "ana pan")).toEqual([]);
    expect(filterNotes(notes, "   ")).toEqual(notes);
  });

  it("da formato a la fecha en hora local", () => {
    expect(formatStamp(at(2026, 10, 16, 14, 5))).toBe("16/10/2026 14:05");
  });

  it("exporta a Markdown con un encabezado por nota", () => {
    const md = toMarkdown(notes);
    expect(md.startsWith("# Notas\n\n## 16/10/2026 09:05\n\nComprar café y pan\n")).toBe(true);
    expect(md).toContain("\n---\n\n## 15/10/2026 18:30\n\nLlamar a Ana\nsobre el proyecto\n");
  });

  it("exporta a texto plano sin marcas de Markdown", () => {
    const txt = toPlainText(notes);
    expect(txt).toContain("[16/10/2026 09:05]\nComprar café y pan");
    expect(txt).not.toContain("##");
    expect(exportContent(notes, "txt")).toBe(txt);
    expect(exportContent(notes, "md")).toBe(md(notes));
  });

  it("el nombre del archivo lleva la fecha de hoy y la extensión", () => {
    expect(exportName("md", new Date(2026, 0, 5))).toBe("notas-2026-01-05.md");
    expect(exportName("txt", new Date(2026, 11, 25))).toBe("notas-2026-12-25.txt");
  });

  it("el titular es la primera línea con texto, recortada", () => {
    expect(headline("\n\nHola mundo\nsegunda")).toBe("Hola mundo");
    expect(headline("x".repeat(100), 10)).toBe("xxxxxxxxx…");
  });
});

const md = toMarkdown;
