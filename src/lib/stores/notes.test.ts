import { describe, expect, it } from "vitest";
import type { Note } from "../api/system";
import { describeRange, exportContent, exportFileName, exportName, filterNotes, formatStamp, headline, localDay, presetRange, toMarkdown, toPlainText } from "./notes";

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

describe("notas por fecha", () => {
  const at = (m: number, d: number, h = 12) => new Date(2026, m, d, h, 30).getTime();
  const n = (id: string, text: string, created: number) => ({ id, text, created, updated: created });
  const notes = [n("1", "uno", at(0, 31)), n("2", "dos", at(1, 1)), n("3", "tres", at(1, 15, 23)), n("4", "cuatro", at(2, 1, 0))];

  it("el día de una nota es el local, con ceros a la izquierda", () => {
    expect(localDay(at(0, 5))).toBe("2026-01-05");
    expect(localDay(new Date(2026, 11, 31, 23, 59).getTime())).toBe("2026-12-31");
  });

  it("el rango incluye los dos días y admite límites sueltos", () => {
    const ids = (r: { from?: string; to?: string }) => filterNotes(notes, "", r).map((x) => x.id);
    expect(ids({ from: "2026-02-01", to: "2026-02-15" })).toEqual(["2", "3"]);
    expect(ids({ from: "2026-02-15" })).toEqual(["3", "4"]);
    expect(ids({ to: "2026-01-31" })).toEqual(["1"]);
    expect(ids({})).toEqual(["1", "2", "3", "4"]);
    expect(ids({ from: "2026-04-01" })).toEqual([]);
  });

  it("la fecha se puede escribir en la búsqueda, como en la nota o en formato ISO", () => {
    const ids = (q: string) => filterNotes(notes, q).map((x) => x.id);
    expect(ids("15/02/2026")).toEqual(["3"]);
    expect(ids("2026-02-01")).toEqual(["2"]);
    expect(ids("/02/2026")).toEqual(["2", "3"]);
    expect(ids("dos 01/02")).toEqual(["2"]);
    expect(ids("tres 01/02")).toEqual([]);
  });

  it("el texto y el rango se combinan", () => {
    expect(filterNotes(notes, "tres", { from: "2026-02-01" }).map((x) => x.id)).toEqual(["3"]);
    expect(filterNotes(notes, "uno", { from: "2026-02-01" })).toEqual([]);
  });

  it("el nombre del archivo: una nota lleva su fecha y hora; varias, la de hoy", () => {
    expect(exportFileName([notes[2]], "md", new Date(2030, 0, 1))).toBe("nota-2026-02-15-2330.md");
    expect(exportFileName([notes[0], notes[1]], "txt", new Date(2030, 0, 1))).toBe("notas-2030-01-01.txt");
    expect(exportFileName([], "md", new Date(2030, 0, 1))).toBe("notas-2030-01-01.md");
  });
});

describe("atajos de fecha", () => {
  const now = new Date(2026, 9, 16, 15, 30); // 16/10/2026

  it("hoy, ayer, 7 días y este mes", () => {
    expect(presetRange("today", now)).toEqual({ from: "2026-10-16", to: "2026-10-16" });
    expect(presetRange("yesterday", now)).toEqual({ from: "2026-10-15", to: "2026-10-15" });
    expect(presetRange("week", now)).toEqual({ from: "2026-10-10", to: "2026-10-16" });
    expect(presetRange("month", now)).toEqual({ from: "2026-10-01", to: "2026-10-16" });
  });

  it("los atajos cruzan bien el cambio de mes y de año", () => {
    const jan = new Date(2027, 0, 3, 9, 0);
    expect(presetRange("week", jan)).toEqual({ from: "2026-12-28", to: "2027-01-03" });
    expect(presetRange("yesterday", new Date(2026, 10, 1, 9, 0)).from).toBe("2026-10-31");
  });

  it("describe el rango para leerlo", () => {
    expect(describeRange({ from: "2026-10-10", to: "2026-10-16" })).toBe("Del 10/10/2026 al 16/10/2026");
    expect(describeRange({ from: "2026-10-10", to: "2026-10-10" })).toBe("El 10/10/2026");
    expect(describeRange({ from: "2026-10-10" })).toBe("Desde el 10/10/2026");
    expect(describeRange({ to: "2026-10-16" })).toBe("Hasta el 16/10/2026");
    expect(describeRange({})).toBe("");
  });
});
