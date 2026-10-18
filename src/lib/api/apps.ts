import { invoke } from "@tauri-apps/api/core";

/** Una aplicación que se puede elegir: instalada (menú Inicio) o abierta ahora. */
export interface AppSuggestion {
  name: string;
  /** Ejecutable de su ventana, sin `.exe` y en minúsculas. */
  exe: string;
  /** Qué se abre: su acceso directo o la ruta del programa. */
  path: string;
  running: boolean;
}

/** Una aplicación que se abre junto con Tesela y el workspace (1 a 9) donde se queda. */
export interface StartupApp {
  name: string;
  path: string;
  exe: string;
  workspace: number;
}

export const appsApi = {
  suggestions: () => invoke<AppSuggestion[]>("app_suggestions"),
  startupList: () => invoke<StartupApp[]>("startup_list"),
  startupSave: (apps: StartupApp[]) => invoke<StartupApp[]>("startup_save", { apps }),
};
