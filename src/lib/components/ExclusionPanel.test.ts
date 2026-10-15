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
