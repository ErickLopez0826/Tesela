import { cleanup, fireEvent, render } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { clearFavicons } from "../stores/favicon";
import SiteIcon from "./SiteIcon.svelte";

beforeEach(clearFavicons);
afterEach(cleanup);

const img = () => document.querySelector("img.site") as HTMLImageElement | null;

describe("SiteIcon", () => {
  it("empieza con el favicon de la propia página, sin enviar el referrer", () => {
    render(SiteIcon, { site: "https://claude.ai" });
    expect(img()!.getAttribute("src")).toBe("https://claude.ai/favicon.ico");
    expect(img()!.getAttribute("referrerpolicy")).toBe("no-referrer");
  });

  it("si la página no tiene favicon pasa al servicio de iconos y, si tampoco, al icono genérico", async () => {
    const { container } = render(SiteIcon, { site: "https://claude.ai", fallback: "globe" });
    await fireEvent.error(img()!);
    expect(img()!.getAttribute("src")).toContain("google.com/s2/favicons?domain=claude.ai");
    await fireEvent.error(img()!);
    expect(img()).toBeNull();
    expect(container.querySelector("svg")).not.toBeNull();
  });

  it("una página local no tiene más fuente que la suya", async () => {
    const { container } = render(SiteIcon, { site: "http://localhost:3000" });
    await fireEvent.error(img()!);
    expect(img()).toBeNull();
    expect(container.querySelector("svg")).not.toBeNull();
  });

  it("recuerda lo que falló: la próxima vez no vuelve a probar el favicon que no existe", async () => {
    const first = render(SiteIcon, { site: "https://sinlogo.com" });
    await fireEvent.error(img()!);
    expect(img()!.getAttribute("src")).toContain("s2/favicons");
    first.unmount();
    render(SiteIcon, { site: "https://sinlogo.com" });
    expect(img()!.getAttribute("src")).toContain("s2/favicons");
  });

  it("al cambiar de página usa la fuente de la nueva", async () => {
    const { rerender } = render(SiteIcon, { site: "https://a.com" });
    await fireEvent.error(img()!);
    await rerender({ site: "https://b.com" });
    expect(img()!.getAttribute("src")).toBe("https://b.com/favicon.ico");
  });

  it("un origen que no es http(s) no carga nada de fuera", () => {
    const { container } = render(SiteIcon, { site: "file:///C:/secreto" });
    expect(img()).toBeNull();
    expect(container.querySelector("svg")).not.toBeNull();
  });
});
