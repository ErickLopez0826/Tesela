import type { ClaudeActivity, UsageView, UsageWindow } from "../api/claude";

/** «2 h 15 min», «3 d 4 h», «menos de 1 min»; `null` si ya pasó. */
export function formatRemaining(resetsAt: number, nowSeconds: number): string | null {
  const secs = resetsAt - nowSeconds;
  if (secs <= 0) return null;
  const minutes = Math.floor(secs / 60);
  if (minutes < 1) return "menos de 1 min";
  const days = Math.floor(minutes / 1440);
  const hours = Math.floor((minutes % 1440) / 60);
  const mins = minutes % 60;
  if (days > 0) return hours > 0 ? `${days} d ${hours} h` : `${days} d`;
  if (hours > 0) return mins > 0 ? `${hours} h ${mins} min` : `${hours} h`;
  return `${mins} min`;
}

/** Nombre largo de cada ventana, para la tarjeta. */
const TITLES: Record<string, string> = { five_hour: "Sesión actual", seven_day: "Todos los modelos", seven_day_opus: "Opus", seven_day_sonnet: "Sonnet" };
export const windowTitle = (w: UsageWindow): string => TITLES[w.id] ?? w.label;

/** «Reinicia en 51 min» si falta menos de un día; si no, el día y la hora («Reinicia jue 12:00 a. m.»). */
export function resetLabel(w: UsageWindow, nowSeconds: number): string {
  if (w.resets_at === null) return "";
  if (w.resets_at <= nowSeconds) return "Reiniciada";
  if (w.resets_at - nowSeconds < 86_400) return `Reinicia en ${formatRemaining(w.resets_at, nowSeconds)}`;
  const d = new Date(w.resets_at * 1000);
  const day = d.toLocaleDateString("es", { weekday: "short" }).replace(".", "");
  const time = d.toLocaleTimeString("es", { hour: "numeric", minute: "2-digit", hour12: true });
  return `Reinicia ${day} ${time}`;
}

/** «hace 3 min», «hace 2 h». */
export function formatAge(fetchedAt: number, nowSeconds: number): string {
  const minutes = Math.max(0, Math.floor((nowSeconds - fetchedAt) / 60));
  if (minutes < 1) return "hace un momento";
  if (minutes < 60) return `hace ${minutes} min`;
  const hours = Math.floor(minutes / 60);
  return hours < 24 ? `hace ${hours} h` : `hace ${Math.floor(hours / 24)} d`;
}

/** Una ventana cuyo reinicio ya pasó: su porcentaje ya no vale y no se enseña como actual. */
export const isOutdated = (w: UsageWindow, nowSeconds: number): boolean => w.resets_at !== null && w.resets_at <= nowSeconds;

/** Las dos ventanas del resumen de la barra: 5 horas y semana. */
export function headline(view: UsageView): UsageWindow[] {
  return view.windows.filter((w) => w.id === "five_hour" || w.id === "seven_day");
}

const SHORT: Record<string, string> = { five_hour: "5 h", seven_day: "Sem" };

/** «5 h 35% · Sem 24%», con «—» donde la ventana ya se reinició y aún no hay lectura nueva. */
export function usageText(view: UsageView, nowSeconds: number): string {
  return headline(view)
    .map((w) => `${SHORT[w.id] ?? w.label} ${isOutdated(w, nowSeconds) ? "—" : `${Math.round(w.percent)}%`}`)
    .join(" · ");
}

/** Nivel del resumen: el peor de las ventanas vigentes. */
export function usageLevel(view: UsageView, nowSeconds: number): "ok" | "warn" | "danger" {
  const live = headline(view).filter((w) => !isOutdated(w, nowSeconds));
  if (live.some((w) => w.level === "danger")) return "danger";
  return live.some((w) => w.level === "warn") ? "warn" : "ok";
}

/** Detalle para el tooltip: cada ventana con su reinicio, el desglose y, si el dato es viejo, de cuándo es y por qué. */
export function describeUsage(view: UsageView, nowSeconds: number): string {
  const lines = view.windows.map((w) => {
    if (isOutdated(w, nowSeconds)) return `${w.label}: ya se reinició`;
    const left = w.resets_at !== null ? formatRemaining(w.resets_at, nowSeconds) : null;
    return `${w.label}: ${Math.round(w.percent)}%${left ? ` · reinicia en ${left}` : ""}`;
  });
  if (view.breakdown.length > 0) lines.push(`Esta semana: ${view.breakdown.map((s) => `${s.name} ${Math.round(s.percent)}%`).join(" · ")}`);
  if (view.fetched_at !== null) lines.push(`Leído ${formatAge(view.fetched_at, nowSeconds)}`);
  if (view.status === "stale" && view.message) lines.push(`Dato viejo: ${view.message}`);
  return lines.join("\n");
}
