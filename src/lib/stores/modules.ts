import { writable } from "svelte/store";
import type { InstallPreview, InstalledModule, Permission } from "../api/modules";
import { modulesApi } from "../api/modules";

export interface ModulesBackend {
  list: () => Promise<InstalledModule[]>;
  preview: (path: string) => Promise<InstallPreview>;
  install: (path: string, granted: Permission[]) => Promise<InstalledModule>;
  uninstall: (id: string) => Promise<void>;
  setGranted: (id: string, granted: Permission[]) => Promise<void>;
  setEnabled: (id: string, enabled: boolean) => Promise<void>;
  onChanged: (cb: () => void) => Promise<() => void>;
}

/** Módulos instalados, siempre al día: se vuelve a leer tras cada cambio y cuando otra ventana lo avisa. */
export function createModulesStore(backend: ModulesBackend = modulesApi) {
  const { subscribe, set } = writable<InstalledModule[]>([]);
  let unlisten: (() => void) | null = null;

  async function refresh() {
    try {
      set(await backend.list());
    } catch {
      // Se conserva la lista anterior.
    }
  }

  async function then<T>(op: Promise<T>): Promise<T> {
    try {
      return await op;
    } finally {
      await refresh();
    }
  }

  return {
    subscribe,
    refresh,
    async start() {
      await refresh();
      unlisten = await backend.onChanged(() => void refresh());
    },
    stop() {
      unlisten?.();
      unlisten = null;
    },
    preview: (path: string) => backend.preview(path),
    install: async (path: string, granted: Permission[]) => void (await then(backend.install(path, granted))),
    uninstall: (id: string) => then(backend.uninstall(id)),
    setGranted: (id: string, granted: Permission[]) => then(backend.setGranted(id, granted)),
    setEnabled: (id: string, enabled: boolean) => then(backend.setEnabled(id, enabled)),
  };
}

export const modulesStore = createModulesStore();
