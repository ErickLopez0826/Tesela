import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import EnginePathSetup from "./EnginePathSetup.svelte";

afterEach(cleanup);

describe("EnginePathSetup", () => {
  it("pide la carpeta y da una ruta de ejemplo de dónde suele estar", () => {
    render(EnginePathSetup, { onSave: vi.fn() });
    const text = document.body.textContent ?? "";
    expect(text).toContain("wallpaper64.exe");
    expect(text).toContain("steamapps");
    const input = screen.getByRole("textbox", { name: "Carpeta de Wallpaper Engine" }) as HTMLInputElement;
    expect(input.placeholder).toBe(String.raw`C:\Program Files (x86)\Steam\steamapps\common\wallpaper_engine`);
    expect((screen.getByRole("button", { name: "Usar esta carpeta" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("guarda la ruta escrita", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(EnginePathSetup, { onSave });
    await fireEvent.input(screen.getByRole("textbox"), { target: { value: String.raw`D:\Steam\steamapps\common\wallpaper_engine` } });
    await fireEvent.click(screen.getByRole("button", { name: "Usar esta carpeta" }));
    await waitFor(() => expect(onSave).toHaveBeenCalledWith(String.raw`D:\Steam\steamapps\common\wallpaper_engine`));
  });

  it("si la carpeta no sirve enseña el motivo", async () => {
    const onSave = vi.fn().mockRejectedValue("No hay Wallpaper Engine en «X»");
    render(EnginePathSetup, { onSave });
    await fireEvent.input(screen.getByRole("textbox"), { target: { value: "X" } });
    await fireEvent.click(screen.getByRole("button", { name: "Usar esta carpeta" }));
    expect((await screen.findByRole("alert")).textContent).toContain("No hay Wallpaper Engine");
  });
});
