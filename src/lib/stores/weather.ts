import { derived, readable } from "svelte/store";

/** Mérida, Yucatán. Open-Meteo no pide clave ni cuenta; solo se consulta el tiempo actual y el máximo y mínimo del día. */
const URL =
  "https://api.open-meteo.com/v1/forecast?latitude=20.9674&longitude=-89.5926" +
  "&current=temperature_2m,apparent_temperature,relative_humidity_2m,wind_speed_10m,weather_code" +
  "&daily=temperature_2m_max,temperature_2m_min&timezone=auto&forecast_days=1";
const EVERY_MS = 15 * 60 * 1000;

export interface Weather {
  temperature: number;
  feelsLike: number | null;
  humidity: number | null;
  /** km/h */
  wind: number | null;
  /** Código WMO. */
  code: number;
  max: number | null;
  min: number | null;
}

const num = (v: unknown): number | null => (typeof v === "number" && Number.isFinite(v) ? v : null);

export function parseWeather(json: any): Weather | null {
  const c = json?.current;
  const temperature = num(c?.temperature_2m);
  if (temperature === null) return null;
  return {
    temperature,
    feelsLike: num(c?.apparent_temperature),
    humidity: num(c?.relative_humidity_2m),
    wind: num(c?.wind_speed_10m),
    code: num(c?.weather_code) ?? -1,
    max: num(json?.daily?.temperature_2m_max?.[0]),
    min: num(json?.daily?.temperature_2m_min?.[0]),
  };
}

/** Descripción corta del código WMO de Open-Meteo. */
export function describeWeather(code: number): string {
  if (code === 0) return "Despejado";
  if (code === 1) return "Casi despejado";
  if (code === 2) return "Parcialmente nublado";
  if (code === 3) return "Nublado";
  if (code === 45 || code === 48) return "Niebla";
  if (code >= 51 && code <= 57) return "Llovizna";
  if (code >= 61 && code <= 67) return "Lluvia";
  if (code >= 71 && code <= 77) return "Nieve";
  if (code >= 80 && code <= 82) return "Chubascos";
  if (code === 95) return "Tormenta";
  if (code === 96 || code === 99) return "Tormenta con granizo";
  return "Sin datos";
}

export type WeatherKind = "sun" | "moon" | "partly-sun" | "partly-moon" | "cloud" | "fog" | "drizzle" | "rain" | "snow" | "storm" | "unknown";

/** Qué icono corresponde al código WMO de Open-Meteo. De noche, el sol pasa a luna. */
export function weatherKind(code: number, night = false): WeatherKind {
  if (code === 0 || code === 1) return night ? "moon" : "sun";
  if (code === 2) return night ? "partly-moon" : "partly-sun";
  if (code === 3) return "cloud";
  if (code === 45 || code === 48) return "fog";
  if (code >= 51 && code <= 57) return "drizzle";
  if ((code >= 61 && code <= 67) || (code >= 80 && code <= 82)) return "rain";
  if ((code >= 71 && code <= 77) || code === 85 || code === 86) return "snow";
  if (code >= 95 && code <= 99) return "storm";
  return "unknown";
}

export async function fetchWeather(fetcher: typeof fetch = fetch): Promise<Weather | null> {
  try {
    const r = await fetcher(URL, { signal: AbortSignal.timeout(8000) });
    return r.ok ? parseWeather(await r.json()) : null;
  } catch {
    return null;
  }
}

/** Tiempo actual en Mérida, renovado cada 15 minutos; conserva el último dato si una consulta falla. */
export const merida = readable<Weather | null>(null, (set) => {
  let last: Weather | null = null;
  const tick = () => void fetchWeather().then((w) => set((last = w ?? last)));
  tick();
  const timer = setInterval(tick, EVERY_MS);
  return () => clearInterval(timer);
});

/** Solo la temperatura (°C), para la barra. */
export const meridaTemperature = derived(merida, (w) => w?.temperature ?? null);
