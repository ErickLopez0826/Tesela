import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppAction, PopupApi } from "../api/shell";
import { clearIconCache } from "../stores/taskbar";
import AppPopupView from "./AppPopupView.svelte";

const target = { exe: "discord.exe", name: "Discord", path: "C:/d/Discord.exe" };
const list: AppAction[] = [
  { id: "open", label: "Abrir", danger: false, on: null },
  { id: "discord-mute", label: "Silenciar micrófono", danger: false, on: null },
  { id: "mute", label: "Silenciar app", danger: false, on: false },
  { id: "quit", label: "Cerrar", danger: true, on: null },
];

function api(over: Partial<PopupApi> = {}): PopupApi {
  return {
    actions: vi.fn().mockResolvedValue(list),
    run: vi.fn().mockResolvedValue(undefined),
    exeIcon: vi.fn().mockResolvedValue("data:image/png;base64,AAA"),
    close: vi.fn().mockResolvedValue(undefined),
    resize: vi.fn().mockResolvedValue(undefined),
    ...over,
  };
}

beforeEach(() => clearIconCache());
afterEach(cleanup);

describe("AppPopupView", () => {
  it("muestra el nombre, el icono y las acciones que da el núcleo para esa app", async () => {
    const a = api();
    render(AppPopupView, { props: { target, api: a, openTick: 1 } });
    expect(a.actions).toHaveBeenCalledWith("discord.exe");
    await waitFor(() => expect(screen.getByRole("menuitem", { name: /Silenciar micrófono/ })).toBeTruthy());
    expect(screen.getByText("Discord")).toBeTruthy();
    await waitFor(() => expect(document.querySelector("header img")?.getAttribute("src")).toBe("data:image/png;base64,AAA"));
    expect(a.resize).toHaveBeenCalled();
  });

  it("una acción puntual se ejecuta y cierra la ventana", async () => {
    const a = api();
    render(AppPopupView, { props: { target, api: a, openTick: 1 } });
    await fireEvent.click(await screen.findByRole("menuitem", { name: /Silenciar micrófono/ }));
    expect(a.run).toHaveBeenCalledWith("discord.exe", "discord-mute");
    await waitFor(() => expect(a.close).toHaveBeenCalledTimes(1));
  });

  it("un interruptor se ejecuta, vuelve a leer su estado y la ventana sigue abierta", async () => {
    const after = list.map((x) => (x.id === "mute" ? { ...x, label: "Activar sonido", on: true } : x));
    const actions = vi.fn().mockResolvedValueOnce(list).mockResolvedValueOnce(after);
    const a = api({ actions });
    render(AppPopupView, { props: { target, api: a, openTick: 1 } });
    await fireEvent.click(await screen.findByRole("menuitem", { name: /Silenciar app/ }));
    expect(a.run).toHaveBeenCalledWith("discord.exe", "mute");
    await waitFor(() => expect(screen.getByRole("menuitem", { name: /Activar sonido/ }).getAttribute("aria-checked")).toBe("true"));
    expect(a.close).not.toHaveBeenCalled();
  });

  it("cerrar la app pide confirmación y solo «Sí, cerrar» la cierra", async () => {
    const a = api();
    render(AppPopupView, { props: { target, api: a, openTick: 1 } });
    await fireEvent.click(await screen.findByRole("menuitem", { name: "Cerrar" }));
    expect(a.run).not.toHaveBeenCalled();
    expect(screen.getByText("¿Cerrar Discord?")).toBeTruthy();
    await fireEvent.click(screen.getByRole("menuitem", { name: "No" }));
    expect(a.run).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole("menuitem", { name: "Cerrar" }));
    await fireEvent.click(screen.getByRole("menuitem", { name: "Sí, cerrar" }));
    expect(a.run).toHaveBeenCalledWith("discord.exe", "quit");
  });

  it("muestra el error si la acción falla y no se cierra", async () => {
    const a = api({ run: vi.fn().mockRejectedValue("no se pudo") });
    render(AppPopupView, { props: { target, api: a, openTick: 1 } });
    await fireEvent.click(await screen.findByRole("menuitem", { name: /Silenciar micrófono/ }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("no se pudo"));
    expect(a.close).not.toHaveBeenCalled();
  });

  it("al abrirse otra vez pide las acciones de nuevo y al cerrarse usa su animación de salida", async () => {
    const a = api();
    const { rerender } = render(AppPopupView, { props: { target, api: a, openTick: 1, closeTick: 0 } });
    await screen.findByRole("menuitem", { name: "Abrir" });
    await rerender({ openTick: 2 });
    await waitFor(() => expect(a.actions).toHaveBeenCalledTimes(2));
    await rerender({ closeTick: 1 });
    expect(document.querySelector(".stage")?.classList.contains("closing")).toBe(true);
  });

  it("sin app no revienta", () => {
    render(AppPopupView, { props: { target: null, api: api() } });
    expect(document.querySelector(".box")).toBeTruthy();
  });
});
