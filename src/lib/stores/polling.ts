import { writable } from "svelte/store";

/** Lee un valor cada `intervalMs` mientras esté iniciado; una lectura fallida se ignora y se reintenta. */
export function createPollingStore<T>(read: () => Promise<T>, intervalMs: number) {
  const { subscribe, set } = writable<T | null>(null);
  let timer: ReturnType<typeof setInterval> | null = null;

  async function tick() {
    try {
      set(await read());
    } catch {
      // Un fallo puntual no debe tumbar la interfaz.
    }
  }

  return {
    subscribe,
    /** Fuerza una lectura ahora (p. ej. tras ejecutar una acción). */
    refresh: tick,
    start() {
      if (timer) return;
      void tick();
      timer = setInterval(tick, intervalMs);
    },
    stop() {
      if (timer) clearInterval(timer);
      timer = null;
    },
  };
}
