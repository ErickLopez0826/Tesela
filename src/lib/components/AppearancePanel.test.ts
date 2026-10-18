import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { get } from "svelte/store";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import AppearancePanel from "./AppearancePanel.svelte";
import { DEFAULT_THEME, saveTheme, theme } from "../stores/theme";

describe("AppearancePanel", () => {
  beforeEach(() => saveTheme(DEFAULT_THEME));
  afterEach(cleanup);

  it("un color para todo cambia todos los elementos", async () => {
    render(AppearancePanel);
    await fireEvent.click(screen.getByLabelText("Todo en Cian"));
    const t = get(theme);
    expect([t.accent, t.workspace, t.frame, t.dots]).toEqual(Array(4).fill("#7fe8ff"));
  });

  it("cada elemento tiene su propio color", async () => {
    const { container } = render(AppearancePanel);
    await fireEvent.change(container.querySelector('input[aria-label="Marco animado"]')!, { target: { value: "#ff0000" } });
    expect(get(theme).frame).toBe("#ff0000");
    expect(get(theme).accent).toBe(DEFAULT_THEME.accent);
  });

  it("adaptar al fondo desactiva los colores propios y restablecer vuelve al blanco y negro", async () => {
    const { container } = render(AppearancePanel);
    await fireEvent.click(screen.getByRole("radio", { name: "Adaptar al fondo" }));
    expect(get(theme).mode).toBe("wallpaper");
    expect((container.querySelector('input[aria-label="Acento"]') as HTMLInputElement).disabled).toBe(true);
    await fireEvent.click(screen.getByRole("button", { name: "Restablecer valores" }));
    expect(get(theme)).toEqual(DEFAULT_THEME);
  });

  it("los deslizadores de cristal cambian la transparencia y el desenfoque de la barra y del panel", async () => {
    render(AppearancePanel);
    const slider = (name: string) => screen.getByLabelText(name) as HTMLInputElement;
    await fireEvent.input(slider("Barra superior: Transparencia"), { target: { value: "70" } });
    await fireEvent.input(slider("Barra superior: Desenfoque"), { target: { value: "15" } });
    await fireEvent.input(slider("Panel de información: Transparencia"), { target: { value: "35" } });
    await fireEvent.input(slider("Panel de información: Desenfoque"), { target: { value: "80" } });
    const t = get(theme);
    expect([t.barTransparency, t.barBlur, t.panelTransparency, t.panelBlur]).toEqual([70, 15, 35, 80]);
    // La transparencia tiene tope para que el texto siga leyéndose.
    expect(slider("Barra superior: Transparencia").max).toBe("90");
    expect(screen.getByText("70%")).toBeTruthy();
  });

  it("el marco de la ventana en uso puede ser animado, fijo o quitarse", async () => {
    render(AppearancePanel);
    expect(get(theme).borderStyle).toBe("animated");
    await fireEvent.click(screen.getByRole("radio", { name: "Fijo" }));
    expect(get(theme).borderStyle).toBe("steady");
    expect(screen.getByText(/sin ningún efecto animado/)).toBeTruthy();
    await fireEvent.click(screen.getByRole("radio", { name: "Sin marco" }));
    expect(get(theme).borderStyle).toBe("off");
    await fireEvent.click(screen.getByRole("button", { name: "Restablecer valores" }));
    expect(get(theme).borderStyle).toBe("animated");
  });

  it("restablecer devuelve también el cristal a sus valores", async () => {
    render(AppearancePanel);
    await fireEvent.input(screen.getByLabelText("Panel de información: Desenfoque"), { target: { value: "90" } });
    await fireEvent.click(screen.getByRole("button", { name: "Restablecer valores" }));
    expect(get(theme).panelBlur).toBe(DEFAULT_THEME.panelBlur);
  });
});
