import { describe, expect, it } from "vitest";
import { monthGrid, monthTitle, weekDays } from "./week";

describe("monthGrid", () => {
  it("cubre el mes en semanas de lunes a domingo, con relleno de los meses vecinos", () => {
    // Octubre de 2026 empieza en jueves y acaba en sábado: 5 semanas.
    const weeks = monthGrid(2026, 9, new Date(2026, 9, 19));
    expect(weeks).toHaveLength(5);
    expect(weeks[0].map((c) => c.day)).toEqual([28, 29, 30, 1, 2, 3, 4]);
    expect(weeks[0].map((c) => c.inMonth)).toEqual([false, false, false, true, true, true, true]);
    expect(weeks[4].map((c) => c.day)).toEqual([26, 27, 28, 29, 30, 31, 1]);
    expect(weeks[4][6].inMonth).toBe(false);
  });

  it("marca solo el día de hoy, y solo en su mes", () => {
    const today = weekDays(new Date(2026, 9, 19)).find((d) => d.today)!;
    expect(today.day).toBe(19);
    const cells = monthGrid(2026, 9, new Date(2026, 9, 19)).flat();
    expect(cells.filter((c) => c.today).map((c) => c.day)).toEqual([19]);
    expect(monthGrid(2026, 10, new Date(2026, 9, 19)).flat().some((c) => c.today)).toBe(false);
  });

  it("un mes que empieza en lunes no lleva relleno al principio, y febrero puede ocupar 4 semanas", () => {
    expect(monthGrid(2026, 5, new Date(2026, 5, 1))[0][0]).toEqual({ day: 1, inMonth: true, today: true });
    expect(monthGrid(2027, 1, new Date(2027, 1, 1))).toHaveLength(4);
  });

  it("acepta meses fuera de 0-11 (se desbordan al año vecino)", () => {
    const weeks = monthGrid(2026, 12, new Date(2026, 0, 1)); // enero de 2027
    expect(weeks.flat().filter((c) => c.inMonth).map((c) => c.day)).toHaveLength(31);
    expect(monthTitle(2026, 12)).toBe("Enero de 2027");
  });
});

describe("weekDays", () => {
  it("va de lunes a domingo y marca hoy", () => {
    // Martes 29 de septiembre de 2026.
    const days = weekDays(new Date(2026, 8, 29));
    expect(days.map((d) => d.day)).toEqual([28, 29, 30, 1, 2, 3, 4]);
    expect(days.filter((d) => d.today).map((d) => d.label)).toEqual(["Mar"]);
  });

  it("el domingo cierra la semana", () => {
    const days = weekDays(new Date(2026, 9, 4));
    expect(days[6]).toMatchObject({ label: "Dom", day: 4, today: true });
    expect(days[0].day).toBe(28);
  });
});
