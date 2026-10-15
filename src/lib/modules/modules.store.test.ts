import { get } from "svelte/store";
import { describe, expect, it, vi } from "vitest";
import type { InstalledModule } from "../api/modules";
import { createModulesStore, type ModulesBackend } from "../stores/modules";

const mod = (id: string): InstalledModule => ({
  manifest: {
    id,
    name: id,
    version: "1.0.0",
    author: "x",
    description: "d",
    min_core_version: "0.1.0",
    permissions: [],
    network_hosts: [],
    dependencies: [],
    entry: "index.html",
    ui: { panel: { title: id, height: 100 }, bar: null },
    actions: [],
    shortcuts: [],
  },
  granted: [],
  enabled: true,
});

function backend() {
  let list: InstalledModule[] = [];
  let changed: () => void = () => {};
  const b: ModulesBackend = {
    list: async () => list,
    preview: vi.fn(),
    install: vi.fn(async (path: string) => {
      const m = mod(path);
      list = [...list, m];
      return m;
    }),
    uninstall: vi.fn(async (id: string) => {
      list = list.filter((m) => m.manifest.id !== id);
    }),
    setGranted: vi.fn(async () => {}),
    setEnabled: vi.fn(async () => {}),
    onChanged: async (cb) => {
      changed = cb;
      return () => {};
    },
  };
  return { b, notifyFromAnotherWindow: (next: InstalledModule[]) => { list = next; changed(); } };
}

describe("modules store", () => {
  it("se actualiza tras instalar y desinstalar", async () => {
    const { b } = backend();
    const store = createModulesStore(b);
    await store.start();
    await store.install("hola", ["windows"]);
    expect(get(store).map((m) => m.manifest.id)).toEqual(["hola"]);
    await store.uninstall("hola");
    expect(get(store)).toEqual([]);
  });

  it("se actualiza cuando otra ventana avisa del cambio", async () => {
    const { b, notifyFromAnotherWindow } = backend();
    const store = createModulesStore(b);
    await store.start();
    notifyFromAnotherWindow([mod("nuevo")]);
    await vi.waitFor(() => expect(get(store).map((m) => m.manifest.id)).toEqual(["nuevo"]));
  });

  it("vuelve a leer aunque la operación falle", async () => {
    const { b } = backend();
    b.uninstall = vi.fn().mockRejectedValue("no existe");
    const store = createModulesStore(b);
    await store.start();
    await expect(store.uninstall("x")).rejects.toBe("no existe");
  });
});
