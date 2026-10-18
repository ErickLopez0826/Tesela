import { cleanup, fireEvent, render } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import Slider from "./Slider.svelte";

afterEach(cleanup);

const fillOf = (c: HTMLElement) => (c.querySelector(".slider") as HTMLElement).style.getPropertyValue("--p");

describe("Slider", () => {
  it("al máximo la barra queda llena por completo y en cero vacía", () => {
    const { container, rerender } = render(Slider, { value: 100, label: "Volumen" });
    expect(fillOf(container)).toBe("100%");
    return rerender({ value: 0 }).then(() => expect(fillOf(container)).toBe("0%"));
  });

  it("con un máximo propio (p. ej. 90) llena la barra entera al llegar a él", () => {
    const { container } = render(Slider, { value: 90, max: 90, label: "Transparencia" });
    expect(fillOf(container)).toBe("100%");
    const input = container.querySelector("input") as HTMLInputElement;
    expect(input.max).toBe("90");
  });

  it("no se sale de la pista si el valor está fuera de rango", async () => {
    const { container, rerender } = render(Slider, { value: 250, label: "x" });
    expect(fillOf(container)).toBe("100%");
    await rerender({ value: -20 });
    expect(fillOf(container)).toBe("0%");
  });

  it("avisa del valor al moverlo y al soltarlo", async () => {
    const oninput = vi.fn();
    const onchange = vi.fn();
    const { getByLabelText } = render(Slider, { value: 10, label: "Volumen", oninput, onchange });
    const input = getByLabelText("Volumen") as HTMLInputElement;
    await fireEvent.input(input, { target: { value: "42" } });
    expect(oninput).toHaveBeenCalledWith(42);
    await fireEvent.change(input, { target: { value: "42" } });
    expect(onchange).toHaveBeenCalledWith(42);
  });

  it("tiene círculo y, desactivado, no deja moverlo", () => {
    const { container } = render(Slider, { value: 30, label: "x", disabled: true });
    expect(container.querySelector(".thumb")).toBeTruthy();
    expect((container.querySelector("input") as HTMLInputElement).disabled).toBe(true);
  });
});
