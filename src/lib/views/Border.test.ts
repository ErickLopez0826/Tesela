import { cleanup, render } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import Border from "./Border.svelte";
import { DEFAULT_THEME, saveTheme } from "../stores/theme";

beforeEach(() => saveTheme(DEFAULT_THEME));
afterEach(cleanup);

describe("Border", () => {
  it("animado: anillo que palpita y destello que recorre el contorno", () => {
    const { container } = render(Border);
    expect(container.querySelector(".ring .glow")).toBeTruthy();
    expect(container.querySelector(".ring .trail .sweep")).toBeTruthy();
    expect(container.querySelector(".ring.steady")).toBeNull();
  });

  it("fijo: sigue el anillo pero sin destello ni animación", () => {
    saveTheme({ ...DEFAULT_THEME, borderStyle: "steady" });
    const { container } = render(Border);
    expect(container.querySelector(".ring.steady .glow")).toBeTruthy();
    expect(container.querySelector(".sweep")).toBeNull();
  });

  it("sin marco: no se dibuja nada", () => {
    saveTheme({ ...DEFAULT_THEME, borderStyle: "off" });
    const { container } = render(Border);
    expect(container.querySelector(".ring")).toBeNull();
  });
});
