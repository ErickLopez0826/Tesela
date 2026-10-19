import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Redirect, SearchSettingsApi } from "../api/search";
import SearchSettings from "./SearchSettings.svelte";

const CLAUDE: Redirect = { word: "claude", url: "https://claude.com", kind: "redirect" };
const GOOGLE: Redirect = { word: "g", url: "https://www.google.com/search?q={q}", kind: "search" };

function api(initial: Redirect[] = [], over: Partial<SearchSettingsApi> = {}): SearchSettingsApi {
  return {
    redirectsGet: vi.fn().mockResolvedValue(initial),
    redirectsSet: vi.fn().mockImplementation((list: Redirect[]) => Promise.resolve(list)),
    historyLen: vi.fn().mockResolvedValue(0),
    historyClear: vi.fn().mockResolvedValue(0),
    ...over,
  };
}

afterEach(cleanup);

const fill = async (word: string, url: string) => {
  await fireEvent.input(screen.getByLabelText("Palabra"), { target: { value: word } });
  await fireEvent.input(screen.getByLabelText("Dirección"), { target: { value: url } });
};

describe("SearchSettings", () => {
  it("sin entradas lo dice, y de fábrica no hay ninguna", async () => {
    render(SearchSettings, { api: api() });
    expect(await screen.findByText(/Todavía no hay ninguna/)).toBeTruthy();
  });

  it("enseña cada entrada con su palabra, su tipo y su dirección", async () => {
    render(SearchSettings, { api: api([CLAUDE, GOOGLE]) });
    await screen.findByText("https://claude.com");
    // Solo la lista: el texto de ayuda y el formulario repiten esas palabras.
    const list = within(screen.getByRole("list", { name: "Redirecciones y búsquedas web" }));
    expect(list.getByText("claude")).toBeTruthy();
    expect(list.getByText("g texto")).toBeTruthy(); // una búsqueda se escribe con su texto
    expect(list.getByText("Redirección")).toBeTruthy();
    expect(list.getByText("Búsqueda web")).toBeTruthy();
  });

  it("añadir una redirección guarda la lista completa y vacía el formulario", async () => {
    const a = api([GOOGLE]);
    render(SearchSettings, { api: a });
    await screen.findByText("g texto");
    await fill("  claude ", " https://claude.com ");
    await fireEvent.click(screen.getByRole("button", { name: "Añadir" }));
    await waitFor(() => expect(a.redirectsSet).toHaveBeenCalledWith([GOOGLE, CLAUDE]));
    await waitFor(() => expect((screen.getByLabelText("Palabra") as HTMLInputElement).value).toBe(""));
    expect(screen.getByText("https://claude.com")).toBeTruthy();
  });

  it("el botón de añadir espera a que haya palabra y dirección", async () => {
    render(SearchSettings, { api: api() });
    const add = screen.getByRole("button", { name: "Añadir" }) as HTMLButtonElement;
    expect(add.disabled).toBe(true);
    await fill("claude", "");
    expect(add.disabled).toBe(true);
    await fill("claude", "https://claude.com");
    expect(add.disabled).toBe(false);
  });

  it("si el núcleo rechaza la entrada se enseña el motivo, no cambia la lista y el formulario se conserva", async () => {
    const a = api([CLAUDE], { redirectsSet: vi.fn().mockRejectedValue("la dirección de «mal» debe empezar por http:// o https://") });
    render(SearchSettings, { api: a });
    await screen.findByText("https://claude.com");
    await fill("mal", "file:///C:/x");
    await fireEvent.click(screen.getByRole("button", { name: "Añadir" }));
    expect((await screen.findByRole("alert")).textContent).toContain("http:// o https://");
    expect(screen.queryByText("file:///C:/x")).toBeNull();
    expect((screen.getByLabelText("Palabra") as HTMLInputElement).value).toBe("mal");
  });

  it("cambiar el tipo cambia el ejemplo de la dirección", async () => {
    render(SearchSettings, { api: api() });
    const url = screen.getByLabelText("Dirección") as HTMLInputElement;
    expect(url.placeholder).toBe("https://claude.com");
    await fireEvent.change(screen.getByLabelText("Tipo"), { target: { value: "search" } });
    expect(url.placeholder).toContain("{q}");
  });

  it("editar carga la entrada en el formulario y guardar la reemplaza en su sitio", async () => {
    const a = api([CLAUDE, GOOGLE]);
    render(SearchSettings, { api: a });
    await screen.findByText("https://claude.com");
    await fireEvent.click(screen.getByRole("button", { name: "Editar claude" }));
    expect((screen.getByLabelText("Palabra") as HTMLInputElement).value).toBe("claude");
    await fireEvent.input(screen.getByLabelText("Dirección"), { target: { value: "https://claude.ai" } });
    await fireEvent.click(screen.getByRole("button", { name: "Guardar cambios" }));
    await waitFor(() => expect(a.redirectsSet).toHaveBeenCalledWith([{ ...CLAUDE, url: "https://claude.ai" }, GOOGLE]));
    await waitFor(() => expect(screen.getByRole("button", { name: "Añadir" })).toBeTruthy());
  });

  it("cancelar la edición deja el formulario vacío y no guarda nada", async () => {
    const a = api([CLAUDE]);
    render(SearchSettings, { api: a });
    await screen.findByText("https://claude.com");
    await fireEvent.click(screen.getByRole("button", { name: "Editar claude" }));
    await fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect((screen.getByLabelText("Palabra") as HTMLInputElement).value).toBe("");
    expect(a.redirectsSet).not.toHaveBeenCalled();
  });

  it("quitar guarda la lista sin esa entrada", async () => {
    const a = api([CLAUDE, GOOGLE]);
    render(SearchSettings, { api: a });
    await screen.findByText("https://claude.com");
    await fireEvent.click(screen.getByRole("button", { name: "Quitar claude" }));
    await waitFor(() => expect(a.redirectsSet).toHaveBeenCalledWith([GOOGLE]));
    await waitFor(() => expect(screen.queryByText("https://claude.com")).toBeNull());
  });

  it("subir y bajar reordenan, y los extremos no se pueden mover más", async () => {
    const a = api([CLAUDE, GOOGLE]);
    render(SearchSettings, { api: a });
    await screen.findByText("https://claude.com");
    expect((screen.getByRole("button", { name: "Subir claude" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "Bajar g" }) as HTMLButtonElement).disabled).toBe(true);
    await fireEvent.click(screen.getByRole("button", { name: "Bajar claude" }));
    await waitFor(() => expect(a.redirectsSet).toHaveBeenCalledWith([GOOGLE, CLAUDE]));
  });

  it("el historial dice cuánto recuerda y se puede borrar", async () => {
    const a = api([], { historyLen: vi.fn().mockResolvedValue(5), historyClear: vi.fn().mockResolvedValue(0) });
    render(SearchSettings, { api: a });
    await screen.findByText(/Ahora recuerda 5 elementos/);
    await fireEvent.click(screen.getByRole("button", { name: "Borrar historial" }));
    await screen.findByText(/Ahora no recuerda nada/);
    expect(a.historyClear).toHaveBeenCalledTimes(1);
    expect((screen.getByRole("button", { name: "Borrar historial" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("si no se pueden leer las redirecciones se enseña el error", async () => {
    render(SearchSettings, { api: api([], { redirectsGet: vi.fn().mockRejectedValue("sin núcleo") }) });
    expect((await screen.findByRole("alert")).textContent).toContain("sin núcleo");
  });
});
