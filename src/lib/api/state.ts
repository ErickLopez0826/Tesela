import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface MonitorSnapshot {
  id: number;
  primary: boolean;
  vertical: boolean;
  visible_workspace: number | null;
  bar_visible: boolean;
}

export interface WorkspaceSnapshot {
  number: number;
  monitor: number;
  visible: boolean;
  active: boolean;
  window_count: number;
}

export interface FocusedSnapshot {
  exe: string;
  title: string;
}

export interface StateSnapshot {
  monitors: MonitorSnapshot[];
  workspaces: WorkspaceSnapshot[];
  focused: FocusedSnapshot | null;
  paused: boolean;
  mode: string | null;
}

export interface SystemStats {
  cpu_percent: number;
  ram_used: number;
  ram_total: number;
}

export function getState(): Promise<StateSnapshot | null> {
  return invoke<StateSnapshot | null>("get_state");
}

export function systemStats(): Promise<SystemStats> {
  return invoke<SystemStats>("system_stats");
}

/** Ejecuta una acción de la sección 8 por su nombre (p. ej. "workspace_3"). */
export function runAction(name: string): Promise<void> {
  return invoke<void>("run_action", { name });
}

export function openPanel(monitor: number): Promise<void> {
  return invoke<void>("open_panel", { monitor });
}

export function closePanel(): Promise<void> {
  return invoke<void>("close_panel");
}

/** Ajusta el alto de la ventana del panel al de la pestaña visible. */
export function panelResize(height: number): Promise<void> {
  return invoke("panel_resize", { height });
}

/** El panel desplegable se acaba de mostrar (la ventana se reutiliza, así que no se vuelve a montar). */
export function onPanelShown(callback: () => void): Promise<UnlistenFn> {
  return listen("wm-panel-shown", () => callback());
}

/** El panel empieza a cerrarse: la vista reproduce su animación al revés y la ventana se oculta poco después. */
export function onPanelClosing(callback: () => void): Promise<UnlistenFn> {
  return listen("wm-panel-closing", () => callback());
}

export function onState(callback: (state: StateSnapshot) => void): Promise<UnlistenFn> {
  return listen<StateSnapshot>("wm-state", (e) => callback(e.payload));
}
