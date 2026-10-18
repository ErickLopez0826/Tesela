import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Cuánto de la interfaz de Windows reemplaza Tesela. */
export interface ShellSettings {
  /** Oculta la barra de tareas de Windows y muestra la de Tesela. */
  replace_taskbar: boolean;
  /** Tocar la tecla Windows abre el panel de Tesela en vez del menú Inicio. */
  win_key_panel: boolean;
}

export const shellApi = {
  get: () => invoke<ShellSettings>("shell_get"),
  set: (settings: ShellSettings) => invoke<ShellSettings>("shell_set", { settings }),
};

/** Lo que usa la vista de la barra de tareas. */
export interface TaskbarApi {
  focus: (handle: number) => Promise<void>;
  icon: (handle: number, key: string) => Promise<string | null>;
  resize: (width: number) => Promise<void>;
  /** La barra se cierra (al tocar una app). */
  hide: () => Promise<void>;
}

export const taskbarApi: TaskbarApi = {
  focus: (handle) => invoke<void>("focus_window", { handle }),
  icon: (handle, key) => invoke<string | null>("app_icon", { handle, key }),
  resize: (width) => invoke<void>("taskbar_resize", { width }),
  hide: () => invoke<void>("taskbar_hide"),
};

/** El núcleo desplegó la barra de tareas de este monitor. */
export const onTaskbarShown = (cb: () => void): Promise<UnlistenFn> => listen("wm-taskbar-shown", cb);
/** El núcleo empezó a cerrarla. */
export const onTaskbarClosing = (cb: () => void): Promise<UnlistenFn> => listen("wm-taskbar-closing", cb);
