/** Cuántos arcos tiene el icono de Wi-Fi. */
export const WIFI_ARCS = 3;

/** Barras de señal que da el sistema (0 a 5). */
export const MAX_BARS = 5;

/**
 * Arcos encendidos (0 a 3) del icono de Wi-Fi según la intensidad de la conexión.
 * - `undefined`: no se sabe (el núcleo no la informa): el icono se ve completo.
 * - `null`: el Wi-Fi está encendido pero sin conexión: solo el punto.
 * - 0 a 5 barras: de 1 a 3 arcos (cualquier señal enciende al menos uno).
 */
export function wifiArcs(bars: number | null | undefined): number {
  if (bars === undefined) return WIFI_ARCS;
  if (bars === null || bars <= 0) return 0;
  const clamped = Math.min(MAX_BARS, bars);
  return Math.max(1, Math.round((clamped * WIFI_ARCS) / MAX_BARS));
}

/** Texto de la intensidad para el tooltip: «Señal: 4 de 5». */
export function signalText(bars: number | null | undefined): string {
  if (bars === undefined) return "";
  if (bars === null) return "Sin conexión";
  return `Señal: ${Math.min(MAX_BARS, Math.max(0, bars))} de ${MAX_BARS}`;
}
