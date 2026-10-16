import { describe, expect, it } from "vitest";
import { weekDays } from "./week";

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
