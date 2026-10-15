import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { InstallPreview, InstalledModule } from "../api/modules";
import ModulesPanel from "./ModulesPanel.svelte";

afterEach(cleanup);

const manifest = {
  id: "hello",
  name: "Hola",
  version: "1.0.0",
  author: "Erick",
  description: "Módulo de prueba",
  min_core_version: "0.1.0",
  permissions: ["windows", "files"] as ("windows" | "files")[],
  network_hosts: [],
  dependencies: [],
  entry: "index.html",
  ui: { panel: { title: "Hola", height: 100 }, bar: null },
  actions: [{ id: "saludar", title: "Saludar" }],
  shortcuts: [{ action: "saludar", keys: ["alt+g"] }],
};

const preview: InstallPreview = {
  manifest,
  permissions: [
    { permission: "windows", description: "Ver tus workspaces y la ventana enfocada" },
    { permission: "files", description: "Guardar y leer archivos en su propia carpeta" },
  ],
  network_hosts: [],
  installed_version: null,
  files: 3,
  bytes: 100,
};

const installed: InstalledModule = { manifest, granted: ["windows"], enabled: true };

function props(overrides = {}) {
  return {
    modules: [] as InstalledModule[],
    onPreview: vi.fn().mockResolvedValue(preview),
    onInstall: vi.fn().mockResolvedValue(undefined),
    onUninstall: vi.fn().mockResolvedValue(undefined),
    onSetGranted: vi.fn().mockResolvedValue(undefined),
    onSetEnabled: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

async function review(p: ReturnType<typeof props>) {
  render(ModulesPanel, p);
  await fireEvent.input(screen.getByLabelText("Carpeta del módulo"), { target: { value: "C:\\mods\\hello" } });
  await fireEvent.click(screen.getByText("Revisar"));
  await screen.findByRole("region", { name: "Revisión del módulo" });
}

describe("ModulesPanel: instalar", () => {
  it("muestra los permisos que pide ANTES de instalar, sin nada concedido por defecto", async () => {
    const p = props();
    await review(p);
    expect(p.onPreview).toHaveBeenCalledWith("C:\\mods\\hello");
    expect(screen.getByText(/Ver tus workspaces y la ventana enfocada/)).toBeTruthy();
    expect(screen.getByText(/Guardar y leer archivos en su propia carpeta/)).toBeTruthy();
    expect(screen.getByText("Acciones que aporta: Saludar")).toBeTruthy();
    expect(screen.getByText(/Atajos sugeridos \(no se aplican solos\): alt\+g/)).toBeTruthy();
    for (const box of screen.getAllByRole("checkbox")) expect((box as HTMLInputElement).checked).toBe(false);
    expect(p.onInstall).not.toHaveBeenCalled();
  });

  it("instala solo con los permisos que el usuario marcó", async () => {
    const p = props();
    await review(p);
    await fireEvent.click(screen.getByLabelText(/Ventanas y workspaces/));
    await fireEvent.click(screen.getByText(/Instalar con 1 permiso/));
    await waitFor(() => expect(p.onInstall).toHaveBeenCalledWith("C:\\mods\\hello", ["windows"]));
    await screen.findByText("«Hola» instalado.");
  });

  it("se puede instalar sin conceder nada y cancelar la revisión", async () => {
    const p = props();
    await review(p);
    await fireEvent.click(screen.getByText("Instalar con 0 permisos"));
    await waitFor(() => expect(p.onInstall).toHaveBeenCalledWith("C:\\mods\\hello", []));
    const q = props();
    cleanup();
    await review(q);
    await fireEvent.click(screen.getByText("Cancelar"));
    expect(screen.queryByRole("region", { name: "Revisión del módulo" })).toBeNull();
    expect(q.onInstall).not.toHaveBeenCalled();
  });

  it("muestra los errores de la revisión y de la instalación", async () => {
    const p = props({ onPreview: vi.fn().mockRejectedValue("module.json inválido: falta el campo `id`") });
    render(ModulesPanel, p);
    await fireEvent.input(screen.getByLabelText("Carpeta del módulo"), { target: { value: "C:\\x" } });
    await fireEvent.click(screen.getByText("Revisar"));
    expect((await screen.findByRole("alert")).textContent).toContain("falta el campo");
    cleanup();
    const q = props({ onInstall: vi.fn().mockRejectedValue("necesita el módulo 'base', que no está instalado") });
    await review(q);
    await fireEvent.click(screen.getByText("Instalar con 0 permisos"));
    expect((await screen.findByRole("alert")).textContent).toContain("necesita el módulo 'base'");
  });
});

describe("ModulesPanel: instalados", () => {
  it("lista los módulos con sus permisos concedidos y permite cambiarlos", async () => {
    const p = props({ modules: [installed] });
    render(ModulesPanel, p);
    expect(screen.getByText("Módulos instalados (1)")).toBeTruthy();
    const win = screen.getByLabelText("Ventanas y workspaces") as HTMLInputElement;
    const files = screen.getByLabelText("Archivos propios") as HTMLInputElement;
    expect(win.checked).toBe(true);
    expect(files.checked).toBe(false);
    await fireEvent.click(files);
    expect(p.onSetGranted).toHaveBeenCalledWith("hello", ["windows", "files"]);
    await fireEvent.click(win);
    expect(p.onSetGranted).toHaveBeenCalledWith("hello", []);
  });

  it("desactivar y volver a activar", async () => {
    const p = props({ modules: [installed] });
    render(ModulesPanel, p);
    await fireEvent.click(screen.getByText("Desactivar"));
    expect(p.onSetEnabled).toHaveBeenCalledWith("hello", false);
  });

  it("desinstalar pide confirmación", async () => {
    const p = props({ modules: [installed] });
    render(ModulesPanel, p);
    await fireEvent.click(screen.getByText("Desinstalar"));
    expect(p.onUninstall).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByText("No"));
    await fireEvent.click(screen.getByText("Desinstalar"));
    await fireEvent.click(screen.getByText("Sí, desinstalar"));
    await waitFor(() => expect(p.onUninstall).toHaveBeenCalledWith("hello"));
  });

  it("muestra el motivo si no se puede desinstalar", async () => {
    const p = props({ modules: [installed], onUninstall: vi.fn().mockRejectedValue("no se puede desinstalar 'hello': lo usan otro") });
    render(ModulesPanel, p);
    await fireEvent.click(screen.getByText("Desinstalar"));
    await fireEvent.click(screen.getByText("Sí, desinstalar"));
    expect((await screen.findByRole("alert")).textContent).toContain("lo usan otro");
  });
});
