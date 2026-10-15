import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Permission = "windows" | "power" | "network" | "files" | "notifications" | "system" | "launch" | "clipboard" | "speedtest" | "channels";

export interface ModuleManifest {
  id: string;
  name: string;
  version: string;
  author: string;
  description: string;
  min_core_version: string;
  permissions: Permission[];
  network_hosts: string[];
  dependencies: { id: string; min_version: string | null }[];
  entry: string;
  ui: { panel: { title: string; height: number } | null; bar: { width: number } | null };
  actions: { id: string; title: string }[];
  shortcuts: { action: string; keys: string[] }[];
}

export interface InstalledModule {
  manifest: ModuleManifest;
  granted: Permission[];
  enabled: boolean;
}

export interface InstallPreview {
  manifest: ModuleManifest;
  permissions: { permission: Permission; description: string }[];
  network_hosts: string[];
  installed_version: string | null;
  files: number;
  bytes: number;
}

export interface ModuleEventPayload {
  module: string;
  event: string;
  payload: unknown;
}

export const modulesApi = {
  list: () => invoke<InstalledModule[]>("modules_list"),
  preview: (path: string) => invoke<InstallPreview>("modules_preview", { path }),
  install: (path: string, granted: Permission[]) => invoke<InstalledModule>("modules_install", { path, granted }),
  uninstall: (id: string) => invoke<void>("modules_uninstall", { id }),
  setGranted: (id: string, granted: Permission[]) => invoke<void>("modules_set_granted", { id, granted }),
  setEnabled: (id: string, enabled: boolean) => invoke<void>("modules_set_enabled", { id, enabled }),
  /** Puente de la API del núcleo; el núcleo comprueba los permisos. */
  call: (module: string, method: string, params: unknown) => invoke<unknown>("module_api", { module, method, params }),
  onChanged: (cb: () => void): Promise<UnlistenFn> => listen("wm-modules-changed", () => cb()),
  onEvent: (cb: (e: ModuleEventPayload) => void): Promise<UnlistenFn> =>
    listen<ModuleEventPayload>("wm-module-event", (e) => cb(e.payload)),
};

/**
 * URL con la que el webview carga la página de entrada de un módulo (protocolo `wmmodule`, servido por el núcleo).
 * Termina en `/` para que sus archivos relativos (app.js, __wm/sdk.js...) cuelguen de su propia carpeta.
 * `convertFileSrc` codifica las barras, por eso se le pasa solo el id y la barra final se añade a mano.
 */
export function moduleUrl(id: string): string {
  return `${convertFileSrc(id, "wmmodule")}/`;
}

/** Evento de la ventana para que un botón de acción llegue al iframe del módulo. */
export const MODULE_ACTION_EVENT = "wm-module-action";
