import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SearchApi, SearchItem } from "../api/search";
import SearchView from "./SearchView.svelte";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn().mockResolvedValue(null) }));

const item = (id: string, title: string, kind: SearchItem["kind"] = "app", subtitle = "Aplicación"): SearchItem => ({ id, kind, title, subtitle });

const ITEMS = [item("r1", "Spotify", "running", "En ejecución"), item("r2", "Steam"), item("r3", "Slack")];

function api(over: Partial<SearchApi> = {}): SearchApi {
  return {
    query: vi.fn().mockResolvedValue(ITEMS),
    open: vi.fn().mockResolvedValue(undefined),
    close: vi.fn().mockResolvedValue(undefined),
    resize: vi.fn().mockResolvedValue(undefined),
    icon: vi.fn().mockResolvedValue(null),
    ...over,
  };
}

const box = () => screen.getByRole("combobox") as HTMLInputElement;
const rows = () => screen.queryAllByRole("option");
const keys = (key: string, init: KeyboardEventInit = {}) => fireEvent.keyDown(window, { key, ...init });

afterEach(cleanup);

describe("SearchView", () => {
  it("al abrir pide lo más usado, deja el primer resultado seleccionado y enfoca el cuadro", async () => {
    const a = api();
    render(SearchView, { api: a });
    await waitFor(() => expect(rows()).toHaveLength(3));
    expect(a.query).toHaveBeenCalledWith("");
    expect(rows()[0].getAttribute("aria-selected")).toBe("true");
    expect(rows()[1].getAttribute("aria-selected")).toBe("false");
    expect(document.activeElement).toBe(box());
  });

  it("cada resultado enseña su nombre y su línea secundaria, y las abiertas llevan un punto", async () => {
    render(SearchView, { api: api() });
    await waitFor(() => expect(rows()).toHaveLength(3));
    expect(rows()[0].textContent).toContain("Spotify");
    expect(rows()[0].textContent).toContain("En ejecución");
    expect(rows()[1].textContent).toContain("Aplicación");
    expect(rows()[0].querySelector(".live")).not.toBeNull();
    expect(rows()[1].querySelector(".live")).toBeNull();
  });

  it("escribir busca lo escrito", async () => {
    const a = api();
    render(SearchView, { api: a });
    await fireEvent.input(box(), { target: { value: "st" } });
    expect(a.query).toHaveBeenLastCalledWith("st");
  });

  it("si llega tarde la respuesta de una consulta vieja, se descarta", async () => {
    let slow!: (v: SearchItem[]) => void;
    const query = vi.fn().mockResolvedValueOnce([]).mockImplementationOnce(() => new Promise<SearchItem[]>((r) => (slow = r))).mockResolvedValueOnce([item("n1", "Nueva")]);
    render(SearchView, { api: api({ query }) });
    await fireEvent.input(box(), { target: { value: "a" } });
    await fireEvent.input(box(), { target: { value: "ab" } });
    await waitFor(() => expect(rows()).toHaveLength(1));
    slow([item("v1", "Vieja")]);
    await new Promise((r) => setTimeout(r, 0));
    expect(rows().map((r) => r.textContent)).toEqual([expect.stringContaining("Nueva")]);
  });

  it("las flechas mueven la selección y dan la vuelta en los extremos", async () => {
    render(SearchView, { api: api() });
    await waitFor(() => expect(rows()).toHaveLength(3));
    const selected = () => rows().findIndex((r) => r.getAttribute("aria-selected") === "true");
    await keys("ArrowDown");
    expect(selected()).toBe(1);
    await keys("ArrowDown");
    await keys("ArrowDown");
    expect(selected()).toBe(0);
    await keys("ArrowUp");
    expect(selected()).toBe(2);
  });

  it("Enter abre el resultado elegido y ctrl+Enter lo abre como administrador", async () => {
    const a = api();
    render(SearchView, { api: a });
    await waitFor(() => expect(rows()).toHaveLength(3));
    await keys("ArrowDown");
    await keys("Enter");
    expect(a.open).toHaveBeenLastCalledWith("r2", false);
    await keys("Enter", { ctrlKey: true });
    expect(a.open).toHaveBeenLastCalledWith("r2", true);
  });

  it("Escape cierra el buscador", async () => {
    const a = api();
    render(SearchView, { api: a });
    await keys("Escape");
    expect(a.close).toHaveBeenCalledTimes(1);
  });

  it("hacer clic en un resultado lo abre, y pasar el cursor lo resalta", async () => {
    const a = api();
    render(SearchView, { api: a });
    await waitFor(() => expect(rows()).toHaveLength(3));
    await fireEvent.mouseMove(rows()[2]);
    expect(rows()[2].getAttribute("aria-selected")).toBe("true");
    await fireEvent.mouseDown(rows()[2]);
    expect(a.open).toHaveBeenCalledWith("r3", false);
  });

  it("Enter sin resultados no hace nada", async () => {
    const a = api({ query: vi.fn().mockResolvedValue([]) });
    render(SearchView, { api: a });
    await fireEvent.input(box(), { target: { value: "zzz" } });
    await screen.findByText(/Sin resultados para «zzz»/);
    await keys("Enter");
    expect(a.open).not.toHaveBeenCalled();
  });

  it("si abrir falla se enseña el motivo", async () => {
    const a = api({ open: vi.fn().mockRejectedValue("no se pudo abrir") });
    render(SearchView, { api: a });
    await waitFor(() => expect(rows()).toHaveLength(3));
    await keys("Enter");
    expect((await screen.findByRole("alert")).textContent).toContain("no se pudo abrir");
  });

  it("si la búsqueda falla se enseña el motivo y no queda nada seleccionado", async () => {
    render(SearchView, { api: api({ query: vi.fn().mockRejectedValue("fallo del núcleo") }) });
    expect((await screen.findByRole("alert")).textContent).toContain("fallo del núcleo");
    expect(rows()).toHaveLength(0);
  });

  it("cada vez que se abre vacía el cuadro, vuelve a pedir lo más usado y lo enfoca", async () => {
    const a = api();
    const { rerender } = render(SearchView, { api: a, openTick: 0 });
    await fireEvent.input(box(), { target: { value: "spot" } });
    expect(box().value).toBe("spot");
    box().blur();
    await rerender({ api: a, openTick: 1 });
    await waitFor(() => expect(box().value).toBe(""));
    expect(a.query).toHaveBeenLastCalledWith("");
    expect(document.activeElement).toBe(box());
  });

  it("abrir relanza la animación y cerrar la reproduce al revés", async () => {
    const { rerender } = render(SearchView, { api: api(), openTick: 0, closeTick: 0 });
    const shell = document.querySelector(".shell")!;
    expect(shell.classList.contains("open")).toBe(true);
    await rerender({ api: api(), openTick: 0, closeTick: 1 });
    expect(shell.classList.contains("closing")).toBe(true);
    expect(shell.classList.contains("open")).toBe(false);
    await rerender({ api: api(), openTick: 1, closeTick: 1 });
    expect(shell.classList.contains("open")).toBe(true);
    expect(shell.classList.contains("closing")).toBe(false);
  });

  it("pide el icono de las aplicaciones una sola vez por nombre y usa letras si no llega", async () => {
    const icon = vi.fn().mockImplementation((id: string) => Promise.resolve(id === "r1" ? "data:image/png;base64,AAA" : null));
    const a = api({ icon });
    render(SearchView, { api: a });
    await waitFor(() => expect(rows()[0].querySelector("img")).not.toBeNull());
    expect(rows()[0].querySelector("img")!.getAttribute("src")).toBe("data:image/png;base64,AAA");
    expect(rows()[1].querySelector(".fallback")!.textContent).toBe("S");
    const asked = icon.mock.calls.length;
    await fireEvent.input(box(), { target: { value: "s" } });
    await waitFor(() => expect(a.query).toHaveBeenCalledTimes(2));
    // Spotify ya tiene icono guardado: no se vuelve a pedir.
    expect(icon.mock.calls.filter(([id]) => id === "r1")).toHaveLength(1);
    expect(icon.mock.calls.length).toBeGreaterThanOrEqual(asked);
  });

  it("las redirecciones y las búsquedas web llevan su propio icono y no piden uno al núcleo", async () => {
    const icon = vi.fn().mockResolvedValue(null);
    const found = [item("w1", "Buscar «gatos»", "web_search", "Búsqueda web en google.com"), item("w2", "Abrir claude.com", "redirect", "https://claude.com")];
    render(SearchView, { api: api({ query: vi.fn().mockResolvedValue(found), icon }) });
    await waitFor(() => expect(rows()).toHaveLength(2));
    expect(icon).not.toHaveBeenCalled();
    expect(rows().every((r) => r.querySelector("svg") && !r.querySelector(".fallback"))).toBe(true);
  });

  describe("tamaño de la ventana", () => {
    let original: PropertyDescriptor | undefined;
    beforeEach(() => {
      original = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetHeight");
      Object.defineProperty(HTMLElement.prototype, "offsetHeight", { configurable: true, get: () => 120 });
    });
    afterEach(() => {
      if (original) Object.defineProperty(HTMLElement.prototype, "offsetHeight", original);
      else delete (HTMLElement.prototype as unknown as Record<string, unknown>).offsetHeight;
    });

    it("avisa del alto de su contenido (más el borde) para que la ventana se ajuste", async () => {
      const a = api();
      render(SearchView, { api: a });
      await waitFor(() => expect(a.resize).toHaveBeenCalledWith(122));
    });
  });
});

describe("logos de páginas web", () => {
  it("una redirección y una búsqueda web enseñan el logo de su página y sus apps no", async () => {
    const found = [
      item("w1", "Abrir claude.ai", "redirect", "https://claude.ai"),
      item("w2", "Buscar «gatos»", "web_search", "Búsqueda web en google.com"),
      item("a1", "Steam"),
    ];
    found[0].site = "https://claude.ai";
    found[1].site = "https://www.google.com";
    const icon = vi.fn().mockResolvedValue(null);
    render(SearchView, { api: api({ query: vi.fn().mockResolvedValue(found), icon }) });
    await waitFor(() => expect(rows()).toHaveLength(3));
    expect(rows()[0].querySelector("img")!.getAttribute("src")).toBe("https://claude.ai/favicon.ico");
    expect(rows()[1].querySelector("img")!.getAttribute("src")).toBe("https://www.google.com/favicon.ico");
    expect(rows()[2].querySelector("img")).toBeNull();
    // El núcleo solo da icono a los programas: a las páginas no se les pide.
    expect(icon.mock.calls.map(([id]) => id)).toEqual(["a1"]);
  });

  it("si la página no tiene logo cae al icono genérico de cada tipo", async () => {
    const found = [item("w1", "Abrir sitio.local", "redirect", "http://sitio.local"), item("w2", "Buscar «x»", "web_search", "Búsqueda web en sitio.local")];
    found[0].site = "http://sitio.local";
    found[1].site = "http://sitio.local";
    render(SearchView, { api: api({ query: vi.fn().mockResolvedValue(found) }) });
    await waitFor(() => expect(rows()).toHaveLength(2));
    for (const row of rows()) await fireEvent.error(row.querySelector("img")!);
    expect(rows().every((r) => !r.querySelector("img") && r.querySelector("svg"))).toBe(true);
  });
});
