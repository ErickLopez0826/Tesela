export interface WeekDay {
  label: string;
  day: number;
  today: boolean;
}

const LABELS = ["Lun", "Mar", "Mié", "Jue", "Vie", "Sáb", "Dom"];

/** Los siete días de la semana de `now` (de lunes a domingo), con el de hoy marcado. */
export function weekDays(now: Date): WeekDay[] {
  const offset = (now.getDay() + 6) % 7; // lunes = 0
  return LABELS.map((label, i) => {
    const d = new Date(now.getFullYear(), now.getMonth(), now.getDate() - offset + i);
    return { label, day: d.getDate(), today: i === offset };
  });
}

/** Rango de la semana, p. ej. «28 sep – 4 oct». */
export function weekRange(now: Date): string {
  const offset = (now.getDay() + 6) % 7;
  const start = new Date(now.getFullYear(), now.getMonth(), now.getDate() - offset);
  const end = new Date(now.getFullYear(), now.getMonth(), now.getDate() - offset + 6);
  const f = (d: Date) => d.toLocaleDateString("es", { day: "numeric", month: "short" }).replace(".", "");
  return `${f(start)} – ${f(end)}`;
}

export interface MonthCell {
  day: number;
  /** Pertenece al mes mostrado (y no a los días de relleno del anterior o el siguiente). */
  inMonth: boolean;
  today: boolean;
}

/** Las semanas (de lunes a domingo) que cubren el mes `month` (0-11) de `year`, con los días de relleno de los meses vecinos. */
export function monthGrid(year: number, month: number, now: Date): MonthCell[][] {
  const lead = (new Date(year, month, 1).getDay() + 6) % 7; // huecos antes del día 1 (lunes = 0)
  const count = Math.ceil((lead + new Date(year, month + 1, 0).getDate()) / 7);
  const weeks: MonthCell[][] = [];
  for (let w = 0; w < count; w++) {
    weeks.push(
      Array.from({ length: 7 }, (_, i) => {
        const d = new Date(year, month, 1 - lead + w * 7 + i);
        return {
          day: d.getDate(),
          inMonth: d.getMonth() === ((month % 12) + 12) % 12,
          today: d.getFullYear() === now.getFullYear() && d.getMonth() === now.getMonth() && d.getDate() === now.getDate(),
        };
      }),
    );
  }
  return weeks;
}

/** Título del mes, p. ej. «Octubre 2026». */
export function monthTitle(year: number, month: number): string {
  const text = new Date(year, month, 1).toLocaleDateString("es", { month: "long", year: "numeric" });
  return text.charAt(0).toUpperCase() + text.slice(1);
}

/** Las iniciales de los días, en el orden de la rejilla. */
export const WEEKDAY_LABELS = LABELS;
