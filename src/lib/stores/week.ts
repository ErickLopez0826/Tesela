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
