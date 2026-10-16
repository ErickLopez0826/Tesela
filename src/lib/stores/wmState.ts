import { writable } from "svelte/store";
import * as api from "../api/state";
import { isHidden, onShown } from "./polling";
import type { StateSnapshot, SystemStats, WorkspaceSnapshot } from "../api/state";

/** Workspaces que muestra la barra de un monitor: los suyos que están visibles o tienen ventanas. */
export function workspacesFor(state: StateSnapshot | null, monitorId: number): WorkspaceSnapshot[] {
  if (!state) return [];
  return state.workspaces.filter((w) => w.monitor === monitorId && (w.visible || w.window_count > 0));
}

export function formatPercent(value: number): string {
  return `${Math.round(Math.max(0, Math.min(100, value)))}%`;
}

export function ramPercent(stats: SystemStats): number {
  return stats.ram_total > 0 ? (stats.ram_used / stats.ram_total) * 100 : 0;
}

export function formatGb(bytes: number): string {
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
}

export function formatClock(now: Date, withSeconds = false): string {
  const two = (n: number) => String(n).padStart(2, "0");
  const hour = now.getHours() % 12 || 12;
  const base = `${hour}:${two(now.getMinutes())}`;
  const suffix = now.getHours() < 12 ? "AM" : "PM";
  return `${withSeconds ? `${base}:${two(now.getSeconds())}` : base} ${suffix}`;
}

export interface StateBackend {
  get: () => Promise<StateSnapshot | null>;
  listen: (cb: (s: StateSnapshot) => void) => Promise<() => void>;
}

const tauriBackend: StateBackend = { get: api.getState, listen: api.onState };

/** Estado del escritorio publicado por el núcleo: valor inicial + actualizaciones por evento. */
export function createStateStore(backend: StateBackend = tauriBackend) {
  const { subscribe, set } = writable<StateSnapshot | null>(null);
  let unlisten: (() => void) | null = null;

  async function start() {
    unlisten = await backend.listen((s) => set(s));
    const initial = await backend.get();
    if (initial) set(initial);
  }

  function stop() {
    unlisten?.();
    unlisten = null;
  }

  return { subscribe, start, stop };
}

export interface StatsBackend {
  read: () => Promise<SystemStats>;
}

/** CPU y RAM leídas cada `intervalMs`. */
export function createStatsStore(backend: StatsBackend = { read: api.systemStats }, intervalMs = 2000) {
  const { subscribe, set } = writable<SystemStats | null>(null);
  let timer: ReturnType<typeof setInterval> | null = null;

  async function tick() {
    try {
      set(await backend.read());
    } catch {
      // Una lectura fallida no tumba la barra; se reintenta en el siguiente ciclo.
    }
  }

  let stopListening: (() => void) | null = null;

  function start() {
    if (timer) return;
    void tick();
    timer = setInterval(() => !isHidden() && void tick(), intervalMs);
    stopListening = onShown(tick);
  }

  function stop() {
    if (timer) clearInterval(timer);
    timer = null;
    stopListening?.();
    stopListening = null;
  }

  return { subscribe, start, stop };
}
