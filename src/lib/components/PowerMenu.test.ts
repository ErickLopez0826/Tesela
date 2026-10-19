import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import PowerMenu from "./PowerMenu.svelte";

afterEach(cleanup);

const open = () => fireEvent.click(screen.getByRole("button", { name: "Energía" }));

describe("PowerMenu", () => {
  it("empieza como un solo botón y al pulsarlo muestra las cuatro acciones con su texto", async () => {
    render(PowerMenu, { allowChanges: true, onRun: vi.fn() });
    expect(screen.queryByText("Apagar")).toBeNull();
    await open();
    for (const label of ["Suspender", "Reiniciar", "Hibernar", "Apagar"]) expect(screen.getByText(label)).toBeTruthy();
  });

  it("pide confirmación antes de ejecutar y muestra que fue simulado", async () => {
    const onRun = vi.fn().mockResolvedValue({ simulated: true, description: "Simulado: se habría pedido apagar el equipo" });
    render(PowerMenu, { allowChanges: false, onRun });
    await open();
    expect(screen.getByText("simulado")).toBeTruthy();
    await fireEvent.click(screen.getByText("Apagar"));
    expect(onRun).not.toHaveBeenCalled();
    expect(screen.getByText("¿Apagar?")).toBeTruthy();
    await fireEvent.click(screen.getByText("Sí"));
    expect(onRun).toHaveBeenCalledWith("shutdown");
    await waitFor(() => expect(screen.getByText("Simulado: se habría pedido apagar el equipo")).toBeTruthy());
  });

  it("cancelar no ejecuta nada y vuelve a las acciones", async () => {
    const onRun = vi.fn();
    render(PowerMenu, { allowChanges: true, onRun });
    await open();
    await fireEvent.click(screen.getByText("Reiniciar"));
    await fireEvent.click(screen.getByText("Cancelar"));
    expect(onRun).not.toHaveBeenCalled();
    expect(screen.getByText("Hibernar")).toBeTruthy();
    expect(screen.queryByText("simulado")).toBeNull();
  });

  it("muestra el error si la acción falla", async () => {
    render(PowerMenu, { allowChanges: true, onRun: vi.fn().mockRejectedValue("Windows rechazó la suspensión") });
    await open();
    await fireEvent.click(screen.getByText("Suspender"));
    await fireEvent.click(screen.getByText("Sí"));
    await waitFor(() => expect(screen.getByText("Windows rechazó la suspensión")).toBeTruthy());
  });

  it("«Volver» pliega el menú y deja otra vez el botón", async () => {
    render(PowerMenu, { allowChanges: true, onRun: vi.fn() });
    await open();
    expect(screen.queryByRole("button", { name: "Energía" })).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "Volver a la barra de tareas" }));
    expect(screen.getByRole("button", { name: "Energía" })).toBeTruthy();
    expect(screen.queryByText("Apagar")).toBeNull();
  });

  it("se pliega con el tiempo y al cerrarse la barra", async () => {
    vi.useFakeTimers();
    try {
      const { rerender } = render(PowerMenu, { allowChanges: true, onRun: vi.fn(), closeTick: 0 });
      await open();
      vi.advanceTimersByTime(10100);
      await Promise.resolve();
      expect(screen.queryByText("Apagar")).toBeNull();
      await open();
      await rerender({ closeTick: 1 });
      expect(screen.queryByText("Apagar")).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });
});
