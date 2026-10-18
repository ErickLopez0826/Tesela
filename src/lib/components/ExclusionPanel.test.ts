import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import ExclusionPanel from "./ExclusionPanel.svelte";

const windows = [
  { handle: 1, exe: "juego.exe", title: "Mi juego" },
  { handle: 2, exe: "chrome.exe", title: "Gmail" },
];

afterEach(cleanup);

describe("ExclusionPanel", () => {
  it("excluye una ventana abierta con un clic", async () => {
    const onAdd = vi.fn();
    render(ExclusionPanel, { windows, rules: [], onAdd, onRemove: vi.fn() });
    const buttons = screen.getAllByText("No gestionar");
    await fireEvent.click(buttons[0]);
    expect(onAdd).toHaveBeenCalledWith("juego.exe", null);
  });

  it("marca como excluida la que ya lo está", () => {
    render(ExclusionPanel, { windows, rules: [{ exe: "juego", title_contains: null }], onAdd: vi.fn(), onRemove: vi.fn() });
    expect(screen.getByText("Excluida")).toBeTruthy();
    expect(screen.getAllByText("No gestionar")).toHaveLength(1);
  });

  it("agrega por nombre de proceso con título opcional", async () => {
    const onAdd = vi.fn();
    render(ExclusionPanel, { windows: [], rules: [], onAdd, onRemove: vi.fn() });
    await fireEvent.input(screen.getByLabelText("Nombre del proceso"), { target: { value: "overwatch" } });
    await fireEvent.input(screen.getByLabelText("Criterio de título"), { target: { value: "pip" } });
    await fireEvent.click(screen.getByText("Excluir"));
    expect(onAdd).toHaveBeenCalledWith("overwatch", "pip");
  });

  it("el buscador ofrece las aplicaciones que tienes y al elegir una rellena el proceso", async () => {
    const onAdd = vi.fn();
    const loadApps = vi.fn().mockResolvedValue([
      { name: "Overwatch 2", exe: "overwatch", path: "C:/ow.lnk", running: false },
      { name: "Google Chrome", exe: "chrome", path: "C:/chrome.lnk", running: true },
    ]);
    render(ExclusionPanel, { windows: [], rules: [], onAdd, onRemove: vi.fn(), loadApps });
    const search = screen.getByLabelText("Nombre del proceso");
    await fireEvent.focus(search);
    expect(await screen.findByText("Google Chrome")).toBeTruthy();
    expect(screen.getByText("abierta")).toBeTruthy();
    // Se filtra al escribir, sin importar mayúsculas ni acentos.
    await fireEvent.input(search, { target: { value: "OVER" } });
    expect(screen.queryByText("Google Chrome")).toBeNull();
    await fireEvent.mouseDown(screen.getByText("Overwatch 2"));
    expect((search as HTMLInputElement).value).toBe("overwatch");
    await fireEvent.click(screen.getByText("Excluir"));
    expect(onAdd).toHaveBeenCalledWith("overwatch", null);
  });

  it("también se puede buscar por el nombre del proceso", async () => {
    const loadApps = vi.fn().mockResolvedValue([{ name: "Navegador", exe: "firefox", path: "C:/ff.lnk", running: false }]);
    render(ExclusionPanel, { windows: [], rules: [], onAdd: vi.fn(), onRemove: vi.fn(), loadApps });
    const search = screen.getByLabelText("Nombre del proceso");
    await fireEvent.input(search, { target: { value: "firefox.exe" } });
    expect(await screen.findByText("Navegador")).toBeTruthy();
  });

  it("si no hay coincidencias lo dice, y aun así se puede escribir el proceso a mano", async () => {
    const loadApps = vi.fn().mockResolvedValue([]);
    render(ExclusionPanel, { windows: [], rules: [], onAdd: vi.fn(), onRemove: vi.fn(), loadApps });
    await fireEvent.input(screen.getByLabelText("Nombre del proceso"), { target: { value: "zzz" } });
    expect(await screen.findByText("Ninguna aplicación coincide")).toBeTruthy();
    expect((screen.getByText("Excluir") as HTMLButtonElement).disabled).toBe(false);
  });

  it("las exclusiones se paginan de 10 en 10", async () => {
    const rules = Array.from({ length: 25 }, (_, i) => ({ exe: `app${String(i + 1).padStart(2, "0")}`, title_contains: null }));
    render(ExclusionPanel, { windows: [], rules, onAdd: vi.fn(), onRemove: vi.fn() });
    expect(screen.getByText("Excluidas (25)")).toBeTruthy();
    expect(screen.getAllByText("Quitar").length).toBe(10);
    expect(screen.getByText("app01")).toBeTruthy();
    expect(screen.queryByText("app11")).toBeNull();
    const nav = screen.getByLabelText("Páginas de aplicaciones excluidas");
    expect(nav.textContent).toContain("1 / 3");
    await fireEvent.click(nav.querySelector('button[aria-label="Página siguiente"]')!);
    expect(screen.getByText("app11")).toBeTruthy();
    await fireEvent.click(nav.querySelector('button[aria-label="Página siguiente"]')!);
    expect(screen.getAllByText("Quitar").length).toBe(5);
    expect((nav.querySelector('button[aria-label="Página siguiente"]') as HTMLButtonElement).disabled).toBe(true);
  });

  it("las ventanas abiertas también se paginan de 10 en 10", async () => {
    const many = Array.from({ length: 12 }, (_, i) => ({ handle: i + 1, exe: `p${i + 1}.exe`, title: `Ventana ${i + 1}` }));
    render(ExclusionPanel, { windows: many, rules: [], onAdd: vi.fn(), onRemove: vi.fn() });
    expect(screen.getAllByText("No gestionar").length).toBe(10);
    await fireEvent.click(screen.getByLabelText("Páginas de ventanas abiertas").querySelector('button[aria-label="Página siguiente"]')!);
    expect(screen.getAllByText("No gestionar").length).toBe(2);
  });

  it("con 10 o menos no hay paginación", () => {
    const rules = Array.from({ length: 10 }, (_, i) => ({ exe: `a${i}`, title_contains: null }));
    render(ExclusionPanel, { windows, rules, onAdd: vi.fn(), onRemove: vi.fn() });
    expect(screen.queryByLabelText("Páginas de aplicaciones excluidas")).toBeNull();
  });

  it("quitar la última exclusión de una página vuelve a una página que existe", async () => {
    const rules = Array.from({ length: 11 }, (_, i) => ({ exe: `a${String(i).padStart(2, "0")}`, title_contains: null }));
    const { rerender } = render(ExclusionPanel, { windows: [], rules, onAdd: vi.fn(), onRemove: vi.fn() });
    await fireEvent.click(screen.getByLabelText("Páginas de aplicaciones excluidas").querySelector('button[aria-label="Página siguiente"]')!);
    expect(screen.getAllByText("Quitar").length).toBe(1);
    await rerender({ rules: rules.slice(0, 10) });
    expect(screen.getAllByText("Quitar").length).toBe(10);
  });

  it("no agrega un nombre vacío", () => {
    render(ExclusionPanel, { windows: [], rules: [], onAdd: vi.fn(), onRemove: vi.fn() });
    expect((screen.getByText("Excluir") as HTMLButtonElement).disabled).toBe(true);
  });

  it("quita una exclusión", async () => {
    const onRemove = vi.fn();
    const rule = { exe: "a", title_contains: "x" };
    render(ExclusionPanel, { windows: [], rules: [rule], onAdd: vi.fn(), onRemove });
    await fireEvent.click(screen.getByLabelText("Dejar de excluir a"));
    expect(onRemove).toHaveBeenCalledWith(rule);
  });
});
