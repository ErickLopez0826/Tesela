import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { GeneralStatus } from "../api/general";
import GeneralPanel from "./GeneralPanel.svelte";

afterEach(cleanup);

const base: GeneralStatus = {
  version: "0.1.0",
  managing: true,
  bar: true,
  autostart_enabled: false,
  autostart_command: null,
  config_dir: "C:/Users/x/AppData/Roaming/Tesela",
  bundled_modules_dir: null,
  updater_compiled: false,
};

function props(status: GeneralStatus | null = base, extra = {}) {
  return {
    status,
    onAutostart: vi.fn().mockResolvedValue(undefined),
    onQuit: vi.fn().mockResolvedValue(undefined),
    onRestart: vi.fn().mockResolvedValue(undefined),
    ...extra,
  };
}

describe("GeneralPanel", () => {
  it("tiene la casilla de arranque, las aplicaciones de arranque y los botones para cerrar y reiniciar", () => {
    const { container } = render(GeneralPanel, props());
    expect(screen.getByLabelText(/Iniciar Tesela al entrar en Windows/)).toBeTruthy();
    expect(screen.getByText("Aplicaciones de arranque")).toBeTruthy();
    const buttons = [...container.querySelectorAll("button")].map((b) => b.textContent?.trim());
    expect(buttons).toContain("Cerrar Tesela");
    expect(buttons).toContain("Reiniciar Tesela");
    // Lo demás sigue fuera: ni estado, ni actualizaciones, ni carpetas.
    for (const gone of [/Estado/, /Actualizaciones/, /Carpetas/, /GlazeWM/, /Versión/, /Gestor de ventanas/]) expect(screen.queryByText(gone)).toBeNull();
  });

  it("la casilla activa y desactiva el arranque con Windows", async () => {
    const p = props();
    render(GeneralPanel, p);
    await fireEvent.click(screen.getByLabelText(/Iniciar Tesela al entrar en Windows/));
    expect(p.onAutostart).toHaveBeenCalledWith(true);
  });

  it("las casillas de la interfaz de Windows guardan el cambio y muestran lo que devuelve el guardado", async () => {
    const onShell = vi.fn().mockImplementation(async (v) => v);
    render(GeneralPanel, props(base, { shell: { replace_taskbar: true, win_key_panel: true, show_background: true }, onShell }));
    const taskbar = screen.getByLabelText(/Reemplazar la barra de tareas de Windows/) as HTMLInputElement;
    const winKey = screen.getByLabelText(/La tecla Windows abre el panel/) as HTMLInputElement;
    expect([taskbar.checked, winKey.checked]).toEqual([true, true]);
    await fireEvent.click(taskbar);
    expect(onShell).toHaveBeenLastCalledWith({ replace_taskbar: false, win_key_panel: true, show_background: true });
    await fireEvent.click(winKey);
    expect(onShell).toHaveBeenLastCalledWith({ replace_taskbar: false, win_key_panel: false, show_background: true });
    await waitFor(() => expect(taskbar.checked).toBe(false));
  });

  it("sin los ajustes de Windows (aún no llegaron) no se muestran sus casillas", () => {
    render(GeneralPanel, props());
    expect(screen.queryByLabelText(/Reemplazar la barra de tareas/)).toBeNull();
  });

  it("cerrar Tesela pide confirmación y solo entonces cierra la app", async () => {
    const p = props();
    render(GeneralPanel, p);
    await fireEvent.click(screen.getByRole("button", { name: "Cerrar Tesela" }));
    expect(p.onQuit).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(p.onQuit).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole("button", { name: "Cerrar Tesela" }));
    await fireEvent.click(screen.getByRole("button", { name: "Sí, cerrar" }));
    expect(p.onQuit).toHaveBeenCalledTimes(1);
  });

  it("reiniciar conserva si estaba el gestor de ventanas y la barra", async () => {
    const p = props({ ...base, managing: false, bar: true });
    render(GeneralPanel, p);
    await fireEvent.click(screen.getByRole("button", { name: "Reiniciar Tesela" }));
    expect(p.onRestart).toHaveBeenCalledWith(false, true);
  });

  it("muestra el error si algo falla", async () => {
    render(GeneralPanel, props(base, { onQuit: vi.fn().mockRejectedValue("no se pudo") }));
    await fireEvent.click(screen.getByRole("button", { name: "Cerrar Tesela" }));
    await fireEvent.click(screen.getByRole("button", { name: "Sí, cerrar" }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("no se pudo"));
  });

  describe("aplicaciones de arranque", () => {
    const spotify = { name: "Spotify", path: "C:/lnk/Spotify.lnk", exe: "spotify", workspace: 2 };
    const suggestions = [
      { name: "Spotify", exe: "spotify", path: "C:/lnk/Spotify.lnk", running: false },
      { name: "Visual Studio Code", exe: "code", path: "C:/lnk/Code.lnk", running: true },
    ];
    const withApps = (startupApps: unknown[], extra = {}) => props(base, { startupApps, loadApps: vi.fn().mockResolvedValue(suggestions), onStartupSave: vi.fn(async (a) => a), ...extra });

    it("muestra cada aplicación con su workspace", () => {
      render(GeneralPanel, withApps([spotify]));
      expect(screen.getByText("Spotify")).toBeTruthy();
      expect((screen.getByLabelText("Workspace de Spotify") as HTMLSelectElement).value).toBe("2");
    });

    it("sin aplicaciones lo dice", () => {
      render(GeneralPanel, withApps([]));
      expect(screen.getByText("Ninguna.")).toBeTruthy();
    });

    it("se busca una aplicación, se elige y se agrega con el workspace escogido", async () => {
      const p = withApps([]);
      render(GeneralPanel, p);
      const search = screen.getByLabelText("Aplicación de arranque");
      await fireEvent.focus(search);
      await fireEvent.input(search, { target: { value: "vis" } });
      await fireEvent.mouseDown(await screen.findByText("Visual Studio Code"));
      await fireEvent.change(screen.getByLabelText("Workspace de la nueva aplicación"), { target: { value: "4" } });
      await fireEvent.click(screen.getByRole("button", { name: "Agregar" }));
      expect(p.onStartupSave).toHaveBeenCalledWith([{ name: "Visual Studio Code", path: "C:/lnk/Code.lnk", exe: "code", workspace: 4 }]);
      await waitFor(() => expect((search as HTMLInputElement).value).toBe(""));
    });

    it("no deja agregar un texto que no es una aplicación elegida ni una ruta", async () => {
      render(GeneralPanel, withApps([]));
      await fireEvent.input(screen.getByLabelText("Aplicación de arranque"), { target: { value: "algo" } });
      expect((screen.getByRole("button", { name: "Agregar" }) as HTMLButtonElement).disabled).toBe(true);
    });

    it("acepta una ruta escrita a mano", async () => {
      const p = withApps([]);
      render(GeneralPanel, p);
      await fireEvent.input(screen.getByLabelText("Aplicación de arranque"), { target: { value: "C:\\Juegos\\mi.exe" } });
      await fireEvent.click(screen.getByRole("button", { name: "Agregar" }));
      expect(p.onStartupSave).toHaveBeenCalledWith([{ name: "", path: "C:\\Juegos\\mi.exe", exe: "", workspace: 1 }]);
    });

    it("cambiar el workspace guarda la lista con el cambio", async () => {
      const p = withApps([spotify, { ...spotify, name: "Code", path: "C:/code.lnk", exe: "code", workspace: 1 }]);
      render(GeneralPanel, p);
      await fireEvent.change(screen.getByLabelText("Workspace de Spotify"), { target: { value: "7" } });
      expect(p.onStartupSave).toHaveBeenCalledWith([{ ...spotify, workspace: 7 }, expect.objectContaining({ name: "Code", workspace: 1 })]);
    });

    it("quitar una aplicación guarda la lista sin ella", async () => {
      const p = withApps([spotify]);
      render(GeneralPanel, p);
      await fireEvent.click(screen.getByRole("button", { name: "Quitar Spotify" }));
      expect(p.onStartupSave).toHaveBeenCalledWith([]);
      await waitFor(() => expect(screen.getByText("Ninguna.")).toBeTruthy());
    });

    it("pagina de 10 en 10", async () => {
      const many = Array.from({ length: 23 }, (_, i) => ({ name: `App ${i + 1}`, path: `C:/app${i + 1}.exe`, exe: `app${i + 1}`, workspace: 1 }));
      render(GeneralPanel, withApps(many));
      expect(screen.getAllByRole("button", { name: /^Quitar App/ }).length).toBe(10);
      expect(screen.getByText("1 / 3")).toBeTruthy();
      await fireEvent.click(screen.getByLabelText("Página siguiente"));
      await fireEvent.click(screen.getByLabelText("Página siguiente"));
      expect(screen.getAllByRole("button", { name: /^Quitar App/ }).length).toBe(3);
      expect(screen.getByText("App 21")).toBeTruthy();
    });

    it("si el guardado falla muestra el error y no pierde la lista", async () => {
      render(GeneralPanel, withApps([spotify], { onStartupSave: vi.fn().mockRejectedValue("el workspace debe estar entre 1 y 9") }));
      await fireEvent.click(screen.getByRole("button", { name: "Quitar Spotify" }));
      await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("entre 1 y 9"));
      expect(screen.getByText("Spotify")).toBeTruthy();
    });
  });

  it("mientras carga el estado no muestra controles", () => {
    const { container } = render(GeneralPanel, props(null));
    expect(screen.getByText("Cargando…")).toBeTruthy();
    expect(container.querySelectorAll("button").length).toBe(0);
  });
});
