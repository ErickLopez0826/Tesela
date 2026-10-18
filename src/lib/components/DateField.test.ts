import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it } from "vitest";
import DateField from "./DateField.svelte";

afterEach(cleanup);

const year = String(new Date().getFullYear());
const box = (part: string) => screen.getByLabelText(`Desde: ${part}`) as HTMLInputElement;
const type = (part: string, v: string) => fireEvent.input(box(part), { target: { value: v } });

describe("DateField", () => {
  it("el año ya viene puesto con el actual", () => {
    render(DateField, { value: "", label: "Desde" });
    expect(box("año").value).toBe(year);
    expect(box("día").value).toBe("");
  });

  it("con día y mes completa la fecha del año actual, sin tocar el año", async () => {
    render(DateField, { value: "", label: "Desde" });
    await type("día", "16");
    await type("mes", "10");
    expect((document.querySelector("input.native") as HTMLInputElement).value).toBe(`${year}-10-16`);
  });

  it("pasa solo a la casilla siguiente al completar el día y el mes", async () => {
    render(DateField, { value: "", label: "Desde" });
    await type("día", "16");
    expect(document.activeElement).toBe(box("mes"));
    await type("mes", "10");
    expect(document.activeElement).toBe(box("año"));
  });

  it("una cifra que no puede empezar un número de dos cifras avanza y se completa con cero", async () => {
    render(DateField, { value: "", label: "Desde" });
    await type("día", "5");
    expect(box("día").value).toBe("05");
    expect(document.activeElement).toBe(box("mes"));
    await type("mes", "3");
    expect(box("mes").value).toBe("03");
    // 1 podría ser 10, 11 o 12: espera.
    cleanup();
    render(DateField, { value: "", label: "Desde" });
    await type("mes", "1");
    expect(box("mes").value).toBe("1");
  });

  it("el año se puede cambiar", async () => {
    render(DateField, { value: "", label: "Desde" });
    await type("día", "02");
    await type("mes", "03");
    await type("año", "2024");
    expect((document.querySelector("input.native") as HTMLInputElement).value).toBe("2024-03-02");
  });

  it("un día que no existe no da fecha", async () => {
    render(DateField, { value: "", label: "Desde" });
    await type("día", "31");
    await type("mes", "02");
    expect((document.querySelector("input.native") as HTMLInputElement).value).toBe("");
  });

  it("sigue al valor que llega de fuera (un atajo) y vuelve al año actual al vaciarse", async () => {
    const { rerender } = render(DateField, { value: "2025-07-04", label: "Desde" });
    expect(box("día").value).toBe("04");
    expect(box("mes").value).toBe("07");
    expect(box("año").value).toBe("2025");
    await rerender({ value: "" });
    expect(box("día").value).toBe("");
    expect(box("año").value).toBe(year);
  });

  it("solo acepta números", async () => {
    render(DateField, { value: "", label: "Desde" });
    await type("día", "a1");
    expect(box("día").value).toBe("1");
  });

  it("si se deja el año a medias vuelve al actual", async () => {
    render(DateField, { value: "", label: "Desde" });
    await type("año", "20");
    await fireEvent.blur(box("año"));
    expect(box("año").value).toBe(year);
  });
});
