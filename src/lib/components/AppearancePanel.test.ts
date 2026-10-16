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
    await fireEvent.click(screen.getByRole("button", { name: "Restablecer blanco y negro" }));
    expect(get(theme)).toEqual(DEFAULT_THEME);
  });
});
