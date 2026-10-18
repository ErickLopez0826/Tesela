import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { writable, get } from "svelte/store";

/** Colores personalizables de la interfaz. En modo «fondo» todos se derivan del color del fondo de pantalla. */
export type BorderStyle = "animated" | "steady" | "off";
export const BORDER_STYLES: BorderStyle[] = ["animated", "steady", "off"];

export interface Theme {
  mode: "custom" | "wallpaper";
  /** Pestañas activas, botones destacados, volumen, hoy en la semana… */
  accent: string;
  /** Workspace actual en la barra. */
  workspace: string;
  /** Marco animado de la ventana en uso. */
  frame: string;
  /** Cómo se dibuja ese marco: animado (palpita y un destello lo recorre), fijo (sin animación) o sin marco. */
  borderStyle: BorderStyle;
  /** Puntos del fondo y su luz. */
  dots: string;
  /** Cristal de la barra y del panel. Transparencia: 0 = tinte opaco. Desenfoque: 0 = se ve lo de detrás sin difuminar. */
  barTransparency: number;
  barBlur: number;
  panelTransparency: number;
  panelBlur: number;
}

/** Lo más transparente que puede ser el tinte (con menos, el texto no se leería). */
export const MAX_TRANSPARENCY = 90;

export const DEFAULT_THEME: Theme = {
  mode: "custom",
  accent: "#f2f2f2",
  workspace: "#f2f2f2",
  frame: "#ffffff",
  borderStyle: "animated",
  dots: "#ffffff",
  // La barra es de cristal (tinte al 58 % sobre el fondo desenfocado); el panel, opaco.
  barTransparency: 42,
  barBlur: 40,
  panelTransparency: 0,
  panelBlur: 0,
};

const KEY = "wm-theme";
const WALLPAPER_EVERY_MS = 15_000;
const WALLPAPER_CHANGED_KEY = "wm-wallpaper-changed";

export function parseHex(hex: string): [number, number, number] | null {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return null;
  const n = parseInt(m[1], 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

export function toHex([r, g, b]: [number, number, number]): string {
  return "#" + [r, g, b].map((v) => Math.max(0, Math.min(255, Math.round(v))).toString(16).padStart(2, "0")).join("");
}

/** Texto negro o blanco, el que mejor se lee sobre ese color. */
export function onColor(hex: string): string {
  const rgb = parseHex(hex);
  if (!rgb) return "#0a0a0b";
  const [r, g, b] = rgb.map((v) => v / 255);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b > 0.5 ? "#0a0a0b" : "#ffffff";
}

function rgbToHsl([r, g, b]: [number, number, number]): [number, number, number] {
  r /= 255;
  g /= 255;
  b /= 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, l];
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  const h = max === r ? (g - b) / d + (g < b ? 6 : 0) : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return [h * 60, s, l];
}

function hslToRgb([h, s, l]: [number, number, number]): [number, number, number] {
  const k = (n: number) => (n + h / 30) % 12;
  const a = s * Math.min(l, 1 - l);
  const f = (n: number) => l - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)));
  return [f(0) * 255, f(8) * 255, f(4) * 255];
}

/** Color del fondo → color de acento: mismo tono, claro y con algo de viveza para que destaque sobre el negro. */
export function accentFromWallpaper(hex: string): string {
  const rgb = parseHex(hex);
  if (!rgb) return DEFAULT_THEME.accent;
  const [h, s] = rgbToHsl(rgb);
  // Un fondo oscuro y apagado tiene poca viveza, pero su matiz cuenta: se sube la viveza para que el acento se note.
  // Solo un fondo realmente sin color (blanco y negro) da un acento gris.
  return toHex(hslToRgb([h, s > 0.03 ? Math.min(0.85, Math.max(s, 0.55)) : 0, 0.72]));
}

export function sanitize(raw: unknown): Theme {
  const t = (raw && typeof raw === "object" ? raw : {}) as Partial<Theme>;
  const color = (v: unknown, fallback: string) => (typeof v === "string" && parseHex(v) ? v.toLowerCase() : fallback);
  const level = (v: unknown, fallback: number, max: number) => (typeof v === "number" && Number.isFinite(v) ? Math.round(Math.max(0, Math.min(max, v))) : fallback);
  return {
    mode: t.mode === "wallpaper" ? "wallpaper" : "custom",
    accent: color(t.accent, DEFAULT_THEME.accent),
    workspace: color(t.workspace, DEFAULT_THEME.workspace),
    frame: color(t.frame, DEFAULT_THEME.frame),
    borderStyle: BORDER_STYLES.includes(t.borderStyle as BorderStyle) ? (t.borderStyle as BorderStyle) : DEFAULT_THEME.borderStyle,
    dots: color(t.dots, DEFAULT_THEME.dots),
    barTransparency: level(t.barTransparency, DEFAULT_THEME.barTransparency, MAX_TRANSPARENCY),
    barBlur: level(t.barBlur, DEFAULT_THEME.barBlur, 100),
    panelTransparency: level(t.panelTransparency, DEFAULT_THEME.panelTransparency, MAX_TRANSPARENCY),
    panelBlur: level(t.panelBlur, DEFAULT_THEME.panelBlur, 100),
  };
}

function load(): Theme {
  try {
    return sanitize(JSON.parse(localStorage.getItem(KEY) ?? "null"));
  } catch {
    return { ...DEFAULT_THEME };
  }
}

export const theme = writable<Theme>(load());

export function saveTheme(t: Theme) {
  const clean = sanitize(t);
  try {
    localStorage.setItem(KEY, JSON.stringify(clean));
  } catch {
    /* sin almacenamiento: el cambio vale solo para esta sesión */
  }
  theme.set(clean);
}

/** Variables CSS que reciben todas las ventanas. Los colores de puntos y marco van como «r, g, b» para usarlos con alfa. */
export function cssVars(t: Theme, wallpaper: string | null): Record<string, string> {
  const auto = t.mode === "wallpaper" && wallpaper ? accentFromWallpaper(wallpaper) : null;
  const accent = auto ?? t.accent;
  const channels = (hex: string) => (parseHex(hex) ?? [255, 255, 255]).join(", ");
  return {
    "--accent": accent,
    "--on-accent": onColor(accent),
    "--ws": auto ?? t.workspace,
    "--ws-on": onColor(auto ?? t.workspace),
    "--frame": channels(auto ?? t.frame),
    "--dot": channels(auto ?? t.dots),
    // Opacidad del tinte de la barra y del panel (lo contrario de la transparencia).
    "--bar-alpha": String(+(1 - t.barTransparency / 100).toFixed(2)),
    "--panel-alpha": String(+(1 - t.panelTransparency / 100).toFixed(2)),
  };
}

/** `source` dice de dónde viene el cambio de color: de un botón (la ola nace donde se pulsó) o del fondo de pantalla (nace en el centro). */
export function applyTheme(t: Theme, wallpaper: string | null, root: HTMLElement = document.documentElement, source: "theme" | "wallpaper" = "theme") {
  for (const [k, v] of Object.entries(cssVars(t, wallpaper))) root.style.setProperty(k, v);
  if (typeof window !== "undefined") window.dispatchEvent(new CustomEvent("wm-theme-applied", { detail: { source } }));
}

/** Avisa a todas las ventanas de que el fondo cambió, para que vuelvan a leer su color sin esperar al sondeo. */
export function refreshWallpaperTheme() {
  try {
    localStorage.setItem(WALLPAPER_CHANGED_KEY, String(Date.now()));
    window.dispatchEvent(new Event("wm-wallpaper-changed"));
  } catch {
    /* sin almacenamiento: se actualizará con el sondeo */
  }
}

/** Avisa cuando el fondo de pantalla cambió (en esta ventana o en otra). Devuelve cómo dejar de escuchar. */
export function onWallpaperChanged(callback: () => void): () => void {
  const onStorage = (e: StorageEvent) => e.key === WALLPAPER_CHANGED_KEY && callback();
  window.addEventListener("storage", onStorage);
  window.addEventListener("wm-wallpaper-changed", callback);
  return () => {
    window.removeEventListener("storage", onStorage);
    window.removeEventListener("wm-wallpaper-changed", callback);
  };
}

/** Aplica el tema en esta ventana y lo mantiene al día: cambios hechos en otra ventana y cambios del fondo de pantalla. */
export function installTheme(): () => void {
  let wallpaper: string | null = null;
  const apply = (source: "theme" | "wallpaper" = "theme") => applyTheme(get(theme), wallpaper, document.documentElement, source);
  const refreshWallpaper = async () => {
    if (get(theme).mode !== "wallpaper" || document.visibilityState === "hidden") return;
    try {
      wallpaper = await invoke<string | null>("wallpaper_color");
    } catch {
      wallpaper = null;
    }
    apply("wallpaper");
  };
  const unsubscribe = theme.subscribe(() => {
    apply();
    void refreshWallpaper();
  });
  const onStorage = (e: StorageEvent) => {
    if (e.key === KEY) theme.set(load());
    if (e.key === WALLPAPER_CHANGED_KEY) void refreshWallpaper();
  };
  window.addEventListener("storage", onStorage);
  // La ventana que cambió el fondo no recibe su propio evento: se lee de nuevo también aquí.
  const selfRefresh = () => void refreshWallpaper();
  // El borde de la ventana en uso es una sola ventana que cambia de monitor: el núcleo avisa y toma al momento el color
  // del fondo del monitor nuevo.
  let stopMonitor: (() => void) | undefined;
  try {
    void Promise.resolve(listen("wm-border-monitor", () => void refreshWallpaper()))
      .then((un) => (stopMonitor = un))
      .catch(() => {});
  } catch {
    /* sin núcleo (pruebas) */
  }
  window.addEventListener("wm-wallpaper-changed", selfRefresh);
  const timer = setInterval(refreshWallpaper, WALLPAPER_EVERY_MS);
  // Al volver a mostrarse (panel que se abre) se pone al día enseguida: mientras estuvo oculto no se consultó.
  const onVisible = () => document.visibilityState !== "hidden" && void refreshWallpaper();
  document.addEventListener("visibilitychange", onVisible);
  return () => {
    document.removeEventListener("visibilitychange", onVisible);
    stopMonitor?.();
    unsubscribe();
    window.removeEventListener("storage", onStorage);
    window.removeEventListener("wm-wallpaper-changed", selfRefresh);
    clearInterval(timer);
  };
}
