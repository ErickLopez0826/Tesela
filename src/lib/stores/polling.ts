import { writable } from "svelte/store";

/** ¿La ventana está oculta (panel cerrado, borde escondido)? Entonces no se consulta nada. */
export const isHidden = () => typeof document !== "undefined" && document.visibilityState === "hidden";

/** Ejecuta `tick` ya cuando la ventana vuelve a mostrarse; devuelve cómo dejar de escuchar. */
export function onShown(tick: () => void): () => void {
  if (typeof document === "undefined") return () => {};
  const handler = () => !isHidden() && tick();
  document.addEventListener("visibilitychange", handler);
  return () => document.removeEventListener("visibilitychange", handler);
}

/** Lee un valor cada `intervalMs` mientras esté iniciado y la ventana se vea; una lectura fallida se ignora y se reintenta. */
export function createPollingStore<T>(read: () => Promise<T>, intervalMs: number) {
  const { subscribe, set } = writable<T | null>(null);
  let timer: ReturnType<typeof setInterval> | null = null;
  let stopListening: (() => void) | null = null;

  async function tick() {
    try {
      set(await read());
    } catch {
      // Un fallo puntual no debe tumbar la interfaz.
    }
  }

  const tickIfShown = () => {
    if (!isHidden()) void tick();
  };

  return {
    subscribe,
    /** Fuerza una lectura ahora (p. ej. tras ejecutar una acción). */
    refresh: tick,
    start() {
      if (timer) return;
      void tick();
      timer = setInterval(tickIfShown, intervalMs);
      stopListening = onShown(tick);
    },
    stop() {
      if (timer) clearInterval(timer);
      timer = null;
      stopListening?.();
      stopListening = null;
    },
  };
}
