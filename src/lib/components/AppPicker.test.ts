import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import AppPicker from "./AppPicker.svelte";
import Pager from "./Pager.svelte";

afterEach(cleanup);

const apps = [
  { name: "Ábaco", exe: "abaco", path: "C:/abaco.lnk", running: false },
  { name: "Brave", exe: "brave", path: "C:/brave.lnk", running: true },
  { name: "Calculadora", exe: "calc", path: "C:/calc.lnk", running: false },
];

describe("AppPicker", () => {
  it("lee las aplicaciones solo al enfocar y las muestra todas sin escribir nada", async () => {
    const load = vi.fn().mockResolvedValue(apps);
    render(AppPicker, { label: "Buscar", load });
    expect(load).not.toHaveBeenCalled();
    await fireEvent.focus(screen.getByLabelText("Buscar"));
    expect(await screen.findAllByRole("option")).toHaveLength(3);
    expect(load).toHaveBeenCalledTimes(1);
  });

  it("filtra sin distinguir mayúsculas ni acentos", async () => {
    render(AppPicker, { label: "Buscar", load: vi.fn().mockResolvedValue(apps) });
    const input = screen.getByLabelText("Buscar");
    await fireEvent.input(input, { target: { value: "abaco" } });
    expect((await screen.findAllByRole("option")).map((o) => o.textContent)).toEqual([expect.stringContaining("Ábaco")]);
    await fireEvent.input(input, { target: { value: "CALC" } });
    expect(screen.getAllByRole("option")).toHaveLength(1);
  });

  it("con las flechas y Enter se elige una; sin flechas Enter no elige nada", async () => {
    const onPick = vi.fn();
    render(AppPicker, { label: "Buscar", load: vi.fn().mockResolvedValue(apps), onPick });
    const input = screen.getByLabelText("Buscar");
    await fireEvent.focus(input);
    await screen.findAllByRole("option");
    await fireEvent.keyDown(input, { key: "Enter" });
    expect(onPick).not.toHaveBeenCalled();
    await fireEvent.keyDown(input, { key: "ArrowDown" });
    await fireEvent.keyDown(input, { key: "ArrowDown" });
    await fireEvent.keyDown(input, { key: "Enter" });
    expect(onPick).toHaveBeenCalledWith(apps[1]);
  });

  it("Escape cierra la lista", async () => {
    render(AppPicker, { label: "Buscar", load: vi.fn().mockResolvedValue(apps) });
    const input = screen.getByLabelText("Buscar");
    await fireEvent.focus(input);
    await screen.findAllByRole("option");
    await fireEvent.keyDown(input, { key: "Escape" });
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("si no se pueden leer las aplicaciones sigue funcionando como un campo de texto", async () => {
    render(AppPicker, { label: "Buscar", load: vi.fn().mockRejectedValue("fallo") });
    const input = screen.getByLabelText("Buscar") as HTMLInputElement;
    await fireEvent.input(input, { target: { value: "algo" } });
    expect(await screen.findByText("Ninguna aplicación coincide")).toBeTruthy();
    expect(input.value).toBe("algo");
  });
});

describe("Pager", () => {
  it("no se muestra si todo cabe en una página", () => {
    const { container } = render(Pager, { page: 0, pages: 1, onPage: vi.fn() });
    expect(container.querySelector(".pager")).toBeNull();
  });

  it("anterior y siguiente, desactivados en los extremos", async () => {
    const onPage = vi.fn();
    const { rerender } = render(Pager, { page: 0, pages: 3, onPage });
    expect((screen.getByLabelText("Página anterior") as HTMLButtonElement).disabled).toBe(true);
    await fireEvent.click(screen.getByLabelText("Página siguiente"));
    expect(onPage).toHaveBeenCalledWith(1);
    await rerender({ page: 2 });
    expect((screen.getByLabelText("Página siguiente") as HTMLButtonElement).disabled).toBe(true);
    await fireEvent.click(screen.getByLabelText("Página anterior"));
    expect(onPage).toHaveBeenLastCalledWith(1);
    expect(screen.getByText("3 / 3")).toBeTruthy();
  });
});
