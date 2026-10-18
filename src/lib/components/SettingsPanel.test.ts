import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: () => Promise.resolve(() => {}) }));

import SettingsPanel from "./SettingsPanel.svelte";

const status = {
  version: "0.1.0",
  managing: true,
  bar: true,
  autostart_enabled: false,
  autostart_command: null,
  config_dir: "C:/Users/x/AppData/Roaming/Tesela",
  bundled_modules_dir: null,
  updater_compiled: false,
};

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation((cmd: string) => {
    switch (cmd) {
      case "list_windows":
        return Promise.resolve([{ handle: 1, title: "Mi partida", exe: "overwatch.exe" }]);
      case "list_exclusions":
        return Promise.resolve([{ exe: "discord.exe", title_contains: null }]);
      case "add_exclusion":
        return Promise.resolve([{ exe: "discord.exe", title_contains: null }, { exe: "overwatch.exe", title_contains: null }]);
      case "general_status":
        return Promise.resolve(status);
      case "modules_list":
        return Promise.resolve([]);
      default:
        return Promise.resolve(undefined);
    }
  });
});
afterEach(cleanup);

describe("SettingsPanel", () => {
  it("abre en las aplicaciones excluidas, con las ventanas abiertas y las reglas actuales", async () => {
    render(SettingsPanel);
    expect(screen.getByText(/no se mueven ni se ajustan/)).toBeTruthy();
    await waitFor(() => expect(screen.getByText("overwatch.exe")).toBeTruthy());
    await waitFor(() => expect(screen.getByText("discord.exe")).toBeTruthy());
    expect(screen.getByRole("button", { name: "Aplicaciones excluidas" }).getAttribute("aria-pressed")).toBe("true");
  });

  it("excluir una ventana abierta manda la regla al núcleo", async () => {
    render(SettingsPanel);
    await waitFor(() => expect(screen.getByText("overwatch.exe")).toBeTruthy());
    await fireEvent.click(screen.getByRole("button", { name: "No gestionar" }));
    expect(invoke).toHaveBeenCalledWith("add_exclusion", expect.objectContaining({ exe: "overwatch.exe" }));
  });

  it("solo tiene dos apartados: aplicaciones excluidas y general (ya no hay «Módulos instalados»)", async () => {
    const { container } = render(SettingsPanel);
    expect([...container.querySelectorAll("nav button")].map((b) => b.textContent?.trim())).toEqual(["Aplicaciones excluidas", "General"]);
    expect(screen.queryByText("Módulos instalados")).toBeNull();
  });

  it("general trae la casilla de arranque y los botones de cerrar y reiniciar, y cerrar llega al núcleo", async () => {
    render(SettingsPanel);
    await fireEvent.click(screen.getByRole("button", { name: "General" }));
    await waitFor(() => expect(screen.getByLabelText(/Iniciar Tesela al entrar en Windows/)).toBeTruthy());
    await fireEvent.click(screen.getByRole("button", { name: "Cerrar Tesela" }));
    expect(invoke).not.toHaveBeenCalledWith("quit_app");
    await fireEvent.click(screen.getByRole("button", { name: "Sí, cerrar" }));
    expect(invoke).toHaveBeenCalledWith("quit_app");
  });

  it("al abrirse el panel de nuevo se vuelve a leer la lista de ventanas", async () => {
    const { rerender } = render(SettingsPanel, { tick: 0 });
    await waitFor(() => expect(invoke.mock.calls.filter((c) => c[0] === "list_windows").length).toBe(1));
    await rerender({ tick: 1 });
    await waitFor(() => expect(invoke.mock.calls.filter((c) => c[0] === "list_windows").length).toBe(2));
  });
});
