import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Cuánto de la interfaz de Windows reemplaza Tesela. */
export interface ShellSettings {
  /** Oculta la barra de tareas de Windows y muestra la de Tesela. */
  replace_taskbar: boolean;
  /** Tocar la tecla Windows abre el panel de Tesela en vez del menú Inicio. */
  win_key_panel: boolean;
  /** La barra de tareas muestra las aplicaciones en segundo plano (las de la bandeja). */
  show_background: boolean;
}

export const shellApi = {
  get: () => invoke<ShellSettings>("shell_get"),
  set: (settings: ShellSettings) => invoke<ShellSettings>("shell_set", { settings }),
};

/** Lo que usa la vista de la barra de tareas. */
export interface TaskbarApi {
  focus: (handle: number) => Promise<void>;
  icon: (handle: number, key: string) => Promise<string | null>;
  /** Tamaño que necesita el contenido; la ventana crece hacia arriba. */
  resize: (width: number, height: number) => Promise<void>;
  /** La barra se cierra (al tocar una app). */
  hide: () => Promise<void>;
  /** Icono de un ejecutable en marcha (aplicaciones en segundo plano). */
  exeIcon: (path: string) => Promise<string | null>;
  /** Abre o enfoca una aplicación en segundo plano. */
  openBackground: (exe: string) => Promise<void>;
  /** Abre (o cierra, si ya estaba abierta para esa app) la ventana emergente con sus acciones, junto a su botón. */
  openPopup: (app: BackgroundApp, anchor: { x: number; y: number; w: number; h: number }) => Promise<void>;
}

/** Una acción posible con una aplicación en segundo plano. */
export interface AppAction {
  id: string;
  label: string;
  /** Pide confirmación (cerrar la aplicación). */
  danger: boolean;
  /** Si es un interruptor, su estado (por ejemplo, silenciada). */
  on: boolean | null;
}

/** Una aplicación en segundo plano (con icono en la bandeja de Windows) y lo que consume. */
export interface BackgroundApp {
  name: string;
  exe: string;
  path: string;
  cpu_percent: number;
  ram_bytes: number;
  processes: number;
}

export const backgroundApi = {
  list: () => invoke<BackgroundApp[]>("background_apps"),
};

export const taskbarApi: TaskbarApi = {
  focus: (handle) => invoke<void>("focus_window", { handle }),
  icon: (handle, key) => invoke<string | null>("app_icon", { handle, key }),
  resize: (width, height) => invoke<void>("taskbar_resize", { width, height }),
  hide: () => invoke<void>("taskbar_hide"),
  exeIcon: (path) => invoke<string | null>("exe_icon", { path }),
  openBackground: (exe) => invoke<void>("background_open", { exe }),
  openPopup: (app, a) => invoke<void>("popup_open", { exe: app.exe, name: app.name, path: app.path, x: a.x, y: a.y, w: a.w, h: a.h }),
};

/** El núcleo desplegó la barra de tareas de este monitor. */
export const onTaskbarShown = (cb: () => void): Promise<UnlistenFn> => listen("wm-taskbar-shown", cb);
/** El núcleo empezó a cerrarla. */
export const onTaskbarClosing = (cb: () => void): Promise<UnlistenFn> => listen("wm-taskbar-closing", cb);

/** Lo que usa la ventana emergente de una aplicación en segundo plano. */
export interface PopupApi {
  actions: (exe: string) => Promise<AppAction[]>;
  run: (exe: string, id: string) => Promise<void>;
  exeIcon: (path: string) => Promise<string | null>;
  close: () => Promise<void>;
  resize: (width: number, height: number) => Promise<void>;
}

export const popupApi: PopupApi = {
  actions: (exe) => invoke<AppAction[]>("app_actions", { exe }),
  run: (exe, id) => invoke<void>("app_action_run", { exe, id }),
  exeIcon: (path) => invoke<string | null>("exe_icon", { path }),
  close: () => invoke<void>("popup_close"),
  resize: (width, height) => invoke<void>("popup_resize", { width, height }),
};

export interface PopupTarget {
  exe: string;
  name: string;
  path: string;
}

/** El núcleo abrió la ventana emergente para una aplicación. */
export const onPopupOpen = (cb: (t: PopupTarget) => void): Promise<UnlistenFn> => listen<PopupTarget>("wm-popup-open", (e) => cb(e.payload));
/** El núcleo empezó a cerrarla. */
export const onPopupClosing = (cb: () => void): Promise<UnlistenFn> => listen("wm-popup-closing", cb);
