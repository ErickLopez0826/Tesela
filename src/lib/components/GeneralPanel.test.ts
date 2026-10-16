import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { GeneralStatus } from "../api/general";
import GeneralPanel from "./GeneralPanel.svelte";

afterEach(cleanup);

const base: GeneralStatus = {
  version: "0.1.0",
  managing: false,
  bar: false,
  autostart_enabled: false,
  autostart_command: null,
  config_dir: "C:\\Users\\x\\AppData\\Roaming\\window-manager",
  bundled_modules_dir: "C:\\Program Files\\Tesela\\modules",
  updater_compiled: false,
};

function props(status: GeneralStatus | null = base, extra = {}) {
  return {
    status,
    onAutostart: vi.fn().mockResolvedValue(undefined),
    onRestart: vi.fn().mockResolvedValue(undefined),
    onCheckUpdates: vi.fn().mockResolvedValue(null),
    onInstallUpdate: vi.fn().mockResolvedValue(undefined),
    ...extra,
  };
}

describe("GeneralPanel", () => {
  it("muestra el estado, la versión y las carpetas", () => {
    render(GeneralPanel, props());
    expect(screen.getByText("0.1.0")).toBeTruthy();
    expect(screen.getAllByText("inactivo")).toHaveLength(1);
    expect(screen.getByText("inactiva")).toBeTruthy();
    expect(screen.getByText("C:\\Program Files\\Tesela\\modules")).toBeTruthy();
  });

  it("activar el gestor pide confirmación y reinicia con gestor y barra", async () => {
    const p = props();
    render(GeneralPanel, p);
    await fireEvent.click(screen.getByText(/Activar gestor de ventanas y barra/));
    expect(p.onRestart).not.toHaveBeenCalled();
    expect(screen.getByText(/reorganizará/)).toBeTruthy();
    await fireEvent.click(screen.getByText("Sí, activar gestor y barra"));
    expect(p.onRestart).toHaveBeenCalledWith(true, true);
  });

  it("se puede arrancar solo con la barra, o cancelar", async () => {
    const p = props();
    render(GeneralPanel, p);
    await fireEvent.click(screen.getByText("Solo la barra"));
    expect(p.onRestart).toHaveBeenCalledWith(false, true);
    await fireEvent.click(screen.getByText(/Activar gestor de ventanas y barra/));
    await fireEvent.click(screen.getByText("No"));
    expect(screen.queryByText(/reorganizará/)).toBeNull();
  });

  it("con el gestor activo recuerda la tecla de emergencia y permite reiniciar sin gestor", async () => {
    const p = props({ ...base, managing: true, bar: true });
    render(GeneralPanel, p);
    expect(screen.getByText("F12")).toBeTruthy();
    await fireEvent.click(screen.getByText("Reiniciar sin gestor de ventanas"));
    expect(p.onRestart).toHaveBeenCalledWith(false, true);
  });

  it("activa y desactiva el arranque con Windows", async () => {
    const p = props();
    render(GeneralPanel, p);
    const box = screen.getByLabelText(/Iniciar la app al entrar en Windows/) as HTMLInputElement;
    await fireEvent.click(box);
    expect(p.onAutostart).toHaveBeenCalledWith(true);
    cleanup();
    const q = props({ ...base, autostart_enabled: true, autostart_command: '"C:\\wm.exe" --manage all --bar' });
    render(GeneralPanel, q);
    expect(screen.getByText('"C:\\wm.exe" --manage all --bar')).toBeTruthy();
    await fireEvent.click(screen.getByLabelText(/Iniciar la app al entrar en Windows/));
    expect(q.onAutostart).toHaveBeenCalledWith(false);
    expect(screen.getByText(/dos gestores a la vez/)).toBeTruthy();
  });

  it("explica que las actualizaciones aún no están conectadas", () => {
    render(GeneralPanel, props());
    expect(screen.getByText(/todavía no incluye las actualizaciones automáticas/)).toBeTruthy();
    expect(screen.queryByText("Buscar actualizaciones")).toBeNull();
  });

  it("con actualizaciones compiladas busca e instala", async () => {
    const p = props({ ...base, updater_compiled: true }, { onCheckUpdates: vi.fn().mockResolvedValue({ version: "0.2.0", notes: null }) });
    render(GeneralPanel, p);
    await fireEvent.click(screen.getByText("Buscar actualizaciones"));
    await screen.findByText("Hay una versión nueva: 0.2.0");
    await fireEvent.click(screen.getByText("Instalar 0.2.0 y reiniciar"));
    expect(p.onInstallUpdate).toHaveBeenCalled();
  });

  it("dice que ya tiene la última versión", async () => {
    render(GeneralPanel, props({ ...base, updater_compiled: true }));
    await fireEvent.click(screen.getByText("Buscar actualizaciones"));
    await screen.findByText("Ya tienes la última versión.");
  });

  it("muestra los errores", async () => {
    const p = props(base, { onAutostart: vi.fn().mockRejectedValue("no se pudo guardar el arranque") });
    render(GeneralPanel, p);
    await fireEvent.click(screen.getByLabelText(/Iniciar la app al entrar en Windows/));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("no se pudo guardar"));
  });
});
