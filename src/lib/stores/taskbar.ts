import type { StateSnapshot, WindowSnapshot } from "../api/state";

/** Las ventanas de un workspace, tal como se dibujan en la barra de tareas. */
export interface TaskbarGroup {
  workspace: number;
  /** Es el workspace que tiene el foco ahora. */
  active: boolean;
  /** Es el que se ve en este monitor. */
  visible: boolean;
  windows: WindowSnapshot[];
}

/**
 * Grupos de la barra de tareas de un monitor: sus workspaces con ventanas, por orden de número (1, 2, 3…). El workspace
 * visible aparece aunque no tenga ventanas, para que siempre se sepa dónde se está.
 */
export function groupsFor(state: StateSnapshot | null, monitorId: number): TaskbarGroup[] {
  if (!state) return [];
  const windows = state.windows ?? [];
  return state.workspaces
    .filter((w) => w.monitor === monitorId)
    .map((w) => ({
      workspace: w.number,
      active: w.active,
      visible: w.visible,
      windows: windows.filter((x) => x.workspace === w.number),
    }))
    .filter((g) => g.windows.length > 0 || g.visible);
}

/** Qué ventanas comparten icono: las del mismo ejecutable. Las apps de la tienda comparten ejecutable y se piden una a una. */
export function iconKey(w: WindowSnapshot): string {
  return w.exe === "applicationframehost" || w.exe === "" ? `window-${w.id}` : w.exe;
}

/** Texto de ayuda de un botón: el título de la ventana, o el programa si no tiene. */
export function windowLabel(w: WindowSnapshot): string {
  return w.title.trim() || w.exe;
}

/** Texto de ayuda de una aplicación en segundo plano: nombre, CPU y memoria. */
export function backgroundLabel(a: { name: string; cpu_percent: number; ram_bytes: number; processes: number }, bytes: (n: number) => string): string {
  const cpu = a.cpu_percent < 10 && a.cpu_percent > 0 ? a.cpu_percent.toFixed(1) : Math.round(a.cpu_percent);
  const procs = a.processes > 1 ? ` · ${a.processes} procesos` : "";
  return `${a.name} · CPU ${cpu}% · ${bytes(a.ram_bytes)}${procs}`;
}

const icons = new Map<string, Promise<string | null>>();

/** Pide el icono una sola vez por clave. Un fallo no se recuerda (puede que la ventana aún no tuviera icono). */
export function loadIcon(w: WindowSnapshot, ask: (handle: number, key: string) => Promise<string | null>): Promise<string | null> {
  const key = iconKey(w);
  const hit = icons.get(key);
  if (hit) return hit;
  const pending = ask(w.id, key).catch(() => null);
  icons.set(key, pending);
  void pending.then((url) => url === null && icons.get(key) === pending && icons.delete(key));
  return pending;
}

/** Icono de un ejecutable (aplicaciones en segundo plano), pedido una vez por ruta. */
export function loadExeIcon(path: string, ask: (path: string) => Promise<string | null>): Promise<string | null> {
  const key = `exe:${path}`;
  const hit = icons.get(key);
  if (hit) return hit;
  const pending = ask(path).catch(() => null);
  icons.set(key, pending);
  void pending.then((url) => url === null && icons.get(key) === pending && icons.delete(key));
  return pending;
}

export function clearIconCache() {
  icons.clear();
}
