export type BatteryMode = "charging" | "plugged_in" | "on_battery";

export interface BatteryState {
  percent: number;
  mode: BatteryMode;
  /** Minutos que quedan con la batería, si Windows los sabe. */
  minutes_left: number | null;
  /** El ahorro de batería de Windows está activado. */
  saver: boolean;
}

/** Por debajo de este porcentaje, sin cargador, la batería se marca como baja. */
export const LOW_PERCENT = 20;

export const isLow = (b: BatteryState) => b.mode === "on_battery" && b.percent <= LOW_PERCENT;

/** «2 h 5 min», «45 min». */
export function formatMinutes(minutes: number): string {
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  if (h === 0) return `${m} min`;
  return m === 0 ? `${h} h` : `${h} h ${m} min`;
}

/** Texto para el tooltip y el lector de pantalla. */
export function describeBattery(b: BatteryState): string {
  const base = `Batería ${Math.round(b.percent)}%`;
  let state: string;
  if (b.mode === "charging") state = b.percent >= 100 ? "carga completa" : "cargando";
  else if (b.mode === "plugged_in") state = b.percent >= 100 ? "carga completa, conectada" : "conectada, sin cargar";
  else state = b.minutes_left !== null ? `con batería, ${formatMinutes(b.minutes_left)} restantes` : "con batería";
  return `${base} · ${state}${b.saver ? " · ahorro de batería" : ""}`;
}
