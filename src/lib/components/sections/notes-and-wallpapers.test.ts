import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { EngineLibrary, Note, NotesApi } from "../../api/system";
import EngineWallpapers from "./EngineWallpapers.svelte";
import NotesSection from "./NotesSection.svelte";

afterEach(cleanup);

const stamp = new Date(2026, 9, 16, 9, 5).getTime();
const note = (id: string, text: string, created = stamp): Note => ({ id, text, created, updated: created });

function notesApi(initial: Note[] = [], over: Partial<NotesApi> = {}): NotesApi {
  let n = initial.length;
  return {
    list: vi.fn().mockResolvedValue(initial),
    add: vi.fn(async (text: string) => note(`new${++n}`, text.trim())),
    update: vi.fn(async (id: string, text: string) => ({ ...note(id, text), updated: stamp + 1 })),
    remove: vi.fn().mockResolvedValue(undefined),
    exportFile: vi.fn().mockResolvedValue("C:\\Docs\\Window Manager\\Notas\\notas.md"),
    openFolder: vi.fn().mockResolvedValue(undefined),
    clipboardRead: vi.fn().mockResolvedValue("texto del portapapeles"),
    clipboardWrite: vi.fn().mockResolvedValue(undefined),
    ...over,
  };
}

/** Escribe una fecha «AAAA-MM-DD» en las tres casillas de un campo de fecha, como lo haría una persona. */
async function typeDate(label: string, iso: string) {
  const [y, m, d] = iso.split("-");
  await fireEvent.input(screen.getByLabelText(`${label}: día`), { target: { value: d } });
  await fireEvent.input(screen.getByLabelText(`${label}: mes`), { target: { value: m } });
  await fireEvent.input(screen.getByLabelText(`${label}: año`), { target: { value: y } });
}

describe("NotesSection", () => {
  it("agrega una nota con el botón y con Ctrl+Enter", async () => {
    const api = notesApi();
    render(NotesSection, { api });
    const box = screen.getByLabelText("Nueva nota");
    expect((screen.getByRole("button", { name: "Agregar nota" }) as HTMLButtonElement).disabled).toBe(true);
    await fireEvent.input(box, { target: { value: "  comprar pan " } });
    await fireEvent.click(screen.getByRole("button", { name: "Agregar nota" }));
    await waitFor(() => expect(api.add).toHaveBeenCalledWith("  comprar pan "));
    await waitFor(() => expect(screen.getByText("comprar pan")).toBeTruthy());
    expect((box as HTMLTextAreaElement).value).toBe("");
    await fireEvent.input(box, { target: { value: "otra" } });
    await fireEvent.keyDown(box, { key: "Enter", ctrlKey: true });
    await waitFor(() => expect(api.add).toHaveBeenCalledTimes(2));
  });

  it("lo escrito y no guardado se guarda solo al pasar a otra pestaña (al destruirse)", async () => {
    const api = notesApi();
    const view = render(NotesSection, { api });
    await fireEvent.input(screen.getByLabelText("Nueva nota"), { target: { value: "idea a medias" } });
    view.unmount();
    await waitFor(() => expect(api.add).toHaveBeenCalledWith("idea a medias"));
  });

  it("también se guarda solo al ocultarse el panel, y una sola vez", async () => {
    const api = notesApi();
    render(NotesSection, { api });
    await fireEvent.input(screen.getByLabelText("Nueva nota"), { target: { value: "pendiente" } });
    Object.defineProperty(document, "visibilityState", { configurable: true, get: () => "hidden" });
    document.dispatchEvent(new Event("visibilitychange"));
    document.dispatchEvent(new Event("visibilitychange"));
    await waitFor(() => expect(api.add).toHaveBeenCalledTimes(1));
    expect((screen.getByLabelText("Nueva nota") as HTMLTextAreaElement).value).toBe("");
    Object.defineProperty(document, "visibilityState", { configurable: true, get: () => "visible" });
  });

  it("una nota vacía no se guarda al irse", async () => {
    const api = notesApi();
    const view = render(NotesSection, { api });
    await fireEvent.input(screen.getByLabelText("Nueva nota"), { target: { value: "   " } });
    view.unmount();
    await Promise.resolve();
    expect(api.add).not.toHaveBeenCalled();
  });

  it("una edición sin guardar se guarda sola al irse; sin cambios o vacía no toca la nota", async () => {
    const api = notesApi([note("1", "original")]);
    const view = render(NotesSection, { api });
    await waitFor(() => expect(screen.getByText("original")).toBeTruthy());
    await fireEvent.click(screen.getByRole("button", { name: "Editar" }));
    view.unmount();
    await Promise.resolve();
    expect(api.update, "sin cambios no se guarda").not.toHaveBeenCalled();

    const api2 = notesApi([note("1", "original")]);
    const view2 = render(NotesSection, { api: api2 });
    await waitFor(() => expect(screen.getByText("original")).toBeTruthy());
    await fireEvent.click(screen.getByRole("button", { name: "Editar" }));
    await fireEvent.input(screen.getByLabelText("Editar nota"), { target: { value: "original y más" } });
    view2.unmount();
    await waitFor(() => expect(api2.update).toHaveBeenCalledWith("1", "original y más"));
  });

  it("pegar mete el portapapeles en la nota nueva", async () => {
    const api = notesApi();
    render(NotesSection, { api });
    await fireEvent.input(screen.getByLabelText("Nueva nota"), { target: { value: "inicio" } });
    await fireEvent.click(screen.getByRole("button", { name: "Pegar" }));
    await waitFor(() => expect((screen.getByLabelText("Nueva nota") as HTMLTextAreaElement).value).toBe("inicio\ntexto del portapapeles"));
  });

  it("el buscador filtra en vivo y avisa si no hay coincidencias", async () => {
    render(NotesSection, { api: notesApi([note("1", "Comprar café"), note("2", "Llamar a Ana")]) });
    await waitFor(() => expect(screen.getByText("Comprar café")).toBeTruthy());
    await fireEvent.input(screen.getByLabelText("Buscar notas"), { target: { value: "cafe" } });
    expect(screen.queryByText("Llamar a Ana")).toBeNull();
    expect(screen.getByText("Comprar café")).toBeTruthy();
    await fireEvent.input(screen.getByLabelText("Buscar notas"), { target: { value: "zzz" } });
    expect(screen.getByText("Ninguna nota coincide con la búsqueda.")).toBeTruthy();
  });

  it("copiar manda el texto al portapapeles", async () => {
    const api = notesApi([note("1", "Hola")]);
    render(NotesSection, { api });
    await waitFor(() => expect(screen.getByText("Hola")).toBeTruthy());
    await fireEvent.click(screen.getByRole("button", { name: "Copiar" }));
    await waitFor(() => expect(api.clipboardWrite).toHaveBeenCalledWith("Hola"));
  });

  it("borrar pide confirmación antes de borrar", async () => {
    const api = notesApi([note("1", "Hola")]);
    render(NotesSection, { api });
    await waitFor(() => expect(screen.getByText("Hola")).toBeTruthy());
    await fireEvent.click(screen.getByRole("button", { name: "Borrar" }));
    expect(api.remove).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole("button", { name: "¿Seguro?" }));
    await waitFor(() => expect(api.remove).toHaveBeenCalledWith("1"));
    await waitFor(() => expect(screen.queryByText("Hola")).toBeNull());
  });

  it("editar guarda el texto nuevo", async () => {
    const api = notesApi([note("1", "Hola")]);
    render(NotesSection, { api });
    await waitFor(() => expect(screen.getByText("Hola")).toBeTruthy());
    await fireEvent.click(screen.getByRole("button", { name: "Editar" }));
    await fireEvent.input(screen.getByLabelText("Editar nota"), { target: { value: "Hola mundo" } });
    await fireEvent.click(screen.getByRole("button", { name: "Guardar" }));
    await waitFor(() => expect(api.update).toHaveBeenCalledWith("1", "Hola mundo"));
    await waitFor(() => expect(screen.getByText("Hola mundo")).toBeTruthy());
  });

  it("exporta a Markdown y a texto plano solo lo filtrado", async () => {
    const api = notesApi([note("1", "Comprar café"), note("2", "Llamar a Ana")]);
    render(NotesSection, { api });
    await waitFor(() => expect(screen.getByText("Llamar a Ana")).toBeTruthy());
    await fireEvent.input(screen.getByLabelText("Buscar notas"), { target: { value: "ana" } });
    await fireEvent.click(screen.getByRole("button", { name: "Exportar .md" }));
    await waitFor(() => expect(api.exportFile).toHaveBeenCalled());
    const [name, content] = (api.exportFile as ReturnType<typeof vi.fn>).mock.calls[0];
    // Una sola nota (la única encontrada) se guarda como «nota-…» con su fecha y hora.
    expect(name).toBe("nota-2026-10-16-0905.md");
    expect(content).toContain("# Notas");
    expect(content).toContain("Llamar a Ana");
    expect(content).not.toContain("café");
    await fireEvent.click(screen.getByRole("button", { name: "Exportar .txt" }));
    await waitFor(() => expect(api.exportFile).toHaveBeenCalledTimes(2));
    expect((api.exportFile as ReturnType<typeof vi.fn>).mock.calls[1][1]).not.toContain("#");
    await waitFor(() => expect(screen.getByText(/Nota guardada en C:/)).toBeTruthy());
  });

  describe("exportar una, varias o todas", () => {
    const day = (d: number, h = 10) => new Date(2026, 9, d, h, 0).getTime();
    const many = () => [note("1", "Comprar café", day(14)), note("2", "Llamar a Ana", day(15)), note("3", "Pagar la luz", day(16))];
    const exported = (api: NotesApi, call = 0) => (api.exportFile as ReturnType<typeof vi.fn>).mock.calls[call] as [string, string];

    it("sin marcar ninguna se exportan todas", async () => {
      const api = notesApi(many());
      render(NotesSection, { api });
      await waitFor(() => expect(screen.getByText("Pagar la luz")).toBeTruthy());
      expect(screen.getByText("Se exportan todas")).toBeTruthy();
      await fireEvent.click(screen.getByRole("button", { name: "Exportar .md" }));
      await waitFor(() => expect(api.exportFile).toHaveBeenCalled());
      const [name, content] = exported(api);
      expect(name).toMatch(/^notas-\d{4}-\d{2}-\d{2}\.md$/);
      for (const t of ["Comprar café", "Llamar a Ana", "Pagar la luz"]) expect(content).toContain(t);
      await waitFor(() => expect(screen.getByText(/3 notas guardadas en/)).toBeTruthy());
    });

    it("marcando una sola se exporta solo esa, con su propio nombre de archivo", async () => {
      const api = notesApi(many());
      render(NotesSection, { api });
      await waitFor(() => expect(screen.getByText("Pagar la luz")).toBeTruthy());
      await fireEvent.click(screen.getByLabelText("Seleccionar nota del 15/10/2026 10:00"));
      expect(screen.getByText("Se exporta la nota seleccionada")).toBeTruthy();
      await fireEvent.click(screen.getByRole("button", { name: "Exportar .txt" }));
      await waitFor(() => expect(api.exportFile).toHaveBeenCalled());
      const [name, content] = exported(api);
      expect(name).toBe("nota-2026-10-15-1000.txt");
      expect(content).toContain("Llamar a Ana");
      expect(content).not.toContain("café");
      expect(content).not.toContain("la luz");
    });

    it("marcando varias se exportan solo esas, en el orden de la lista", async () => {
      const api = notesApi(many());
      render(NotesSection, { api });
      await waitFor(() => expect(screen.getByText("Pagar la luz")).toBeTruthy());
      await fireEvent.click(screen.getByLabelText("Seleccionar nota del 16/10/2026 10:00"));
      await fireEvent.click(screen.getByLabelText("Seleccionar nota del 14/10/2026 10:00"));
      expect(screen.getByText("Se exportan las 2 seleccionadas")).toBeTruthy();
      await fireEvent.click(screen.getByRole("button", { name: "Exportar .md" }));
      await waitFor(() => expect(api.exportFile).toHaveBeenCalled());
      const [name, content] = exported(api);
      expect(name).toMatch(/^notas-/);
      expect(content.indexOf("Comprar café")).toBeLessThan(content.indexOf("Pagar la luz"));
      expect(content).not.toContain("Llamar a Ana");
    });

    it("«Todas» marca las que se ven y quitar la selección vuelve a exportar todas", async () => {
      const api = notesApi(many());
      render(NotesSection, { api });
      await waitFor(() => expect(screen.getByText("Pagar la luz")).toBeTruthy());
      await fireEvent.click(screen.getByLabelText("Seleccionar las notas que se ven"));
      expect(screen.getByText("Se exportan las 3 seleccionadas")).toBeTruthy();
      await fireEvent.click(screen.getByRole("button", { name: /Quitar selección \(3\)/ }));
      expect(screen.getByText("Se exportan todas")).toBeTruthy();
    });

    it("«Todas las encontradas» solo marca las que cumplen la búsqueda", async () => {
      const api = notesApi(many());
      render(NotesSection, { api });
      await waitFor(() => expect(screen.getByText("Pagar la luz")).toBeTruthy());
      await fireEvent.input(screen.getByLabelText("Buscar notas"), { target: { value: "ana" } });
      await fireEvent.click(screen.getByLabelText("Seleccionar las notas que se ven"));
      expect(screen.getByText("Se exporta la nota seleccionada")).toBeTruthy();
      await fireEvent.input(screen.getByLabelText("Buscar notas"), { target: { value: "" } });
      expect(screen.getByText("Se exporta la nota seleccionada")).toBeTruthy(); // la marca sobrevive al cambiar la búsqueda
    });

    it("copiar usa la selección y borrar una nota la quita de la selección", async () => {
      const api = notesApi(many());
      render(NotesSection, { api });
      await waitFor(() => expect(screen.getByText("Pagar la luz")).toBeTruthy());
      await fireEvent.click(screen.getByLabelText("Seleccionar nota del 14/10/2026 10:00"));
      await fireEvent.click(screen.getByRole("button", { name: "Copiar selección" }));
      await waitFor(() => expect(api.clipboardWrite).toHaveBeenCalled());
      expect((api.clipboardWrite as ReturnType<typeof vi.fn>).mock.calls[0][0]).toContain("Comprar café");
      expect((api.clipboardWrite as ReturnType<typeof vi.fn>).mock.calls[0][0]).not.toContain("Ana");
      const card = screen.getByText("Comprar café").closest("li")!;
      await fireEvent.click(card.querySelector(".danger")!);
      await fireEvent.click(card.querySelector(".danger")!);
      await waitFor(() => expect(screen.getByText("Se exportan todas")).toBeTruthy());
    });
  });

  describe("buscar por fecha", () => {
    const day = (m: number, d: number) => new Date(2026, m, d, 12, 0).getTime();
    const dated = () => [note("1", "de septiembre", day(8, 30)), note("2", "de octubre", day(9, 10)), note("3", "más reciente", day(9, 20))];

    it("con «desde» y «hasta» solo salen las notas de ese rango (los dos días incluidos)", async () => {
      render(NotesSection, { api: notesApi(dated()) });
      await waitFor(() => expect(screen.getByText("más reciente")).toBeTruthy());
      await fireEvent.click(screen.getByRole("button", { name: "Filtrar por fecha" }));
      await typeDate("Desde la fecha", "2026-10-10");
      expect(screen.queryByText("de septiembre")).toBeNull();
      expect(screen.getByText("de octubre")).toBeTruthy();
      await typeDate("Hasta la fecha", "2026-10-10");
      expect(screen.queryByText("más reciente")).toBeNull();
      expect(screen.getByText("de octubre")).toBeTruthy();
      expect(screen.getByText(/Se exportan las 1 encontradas/)).toBeTruthy();
    });

    it("«Quitar fechas» las muestra todas otra vez", async () => {
      render(NotesSection, { api: notesApi(dated()) });
      await waitFor(() => expect(screen.getByText("más reciente")).toBeTruthy());
      await fireEvent.click(screen.getByRole("button", { name: "Filtrar por fecha" }));
      await typeDate("Desde la fecha", "2026-10-15");
      expect(screen.queryByText("de octubre")).toBeNull();
      await fireEvent.click(screen.getByRole("button", { name: "Quitar fechas" }));
      expect(screen.getByText("de septiembre")).toBeTruthy();
      expect(screen.queryByRole("button", { name: "Quitar fechas" })).toBeNull();
    });

    it("los atajos ponen el rango y se marcan; tocar el activo lo quita", async () => {
      const now = Date.now();
      const recent = [note("1", "de hoy", now), note("2", "viejita", new Date(2020, 0, 5, 12).getTime())];
      render(NotesSection, { api: notesApi(recent) });
      await waitFor(() => expect(screen.getByText("de hoy")).toBeTruthy());
      await fireEvent.click(screen.getByRole("button", { name: "Filtrar por fecha" }));
      await fireEvent.click(screen.getByRole("button", { name: "Hoy" }));
      expect(screen.getByRole("button", { name: "Hoy" }).getAttribute("aria-pressed")).toBe("true");
      expect(screen.queryByText("viejita")).toBeNull();
      expect(screen.getByText(/1 nota$/)).toBeTruthy();
      await fireEvent.click(screen.getByRole("button", { name: "Hoy" }));
      expect(screen.getByText("viejita")).toBeTruthy();
      expect(screen.getByRole("button", { name: "Hoy" }).getAttribute("aria-pressed")).toBe("false");
    });

    it("con un rango puesto el panel de fechas no se puede perder de vista y el botón lo marca", async () => {
      render(NotesSection, { api: notesApi(dated()) });
      await waitFor(() => expect(screen.getByText("más reciente")).toBeTruthy());
      const toggle = screen.getByRole("button", { name: "Filtrar por fecha" });
      expect(screen.queryByLabelText("Desde la fecha")).toBeNull();
      await fireEvent.click(toggle);
      await typeDate("Desde la fecha", "2026-10-10");
      await fireEvent.click(toggle);
      expect(screen.getByLabelText("Desde la fecha")).toBeTruthy();
      expect(toggle.classList.contains("active")).toBe(true);
      expect(screen.getByText(/Desde el 10\/10\/2026 · 2 notas/)).toBeTruthy();
    });

    it("también se encuentra escribiendo la fecha en el buscador", async () => {
      render(NotesSection, { api: notesApi(dated()) });
      await waitFor(() => expect(screen.getByText("más reciente")).toBeTruthy());
      await fireEvent.input(screen.getByLabelText("Buscar notas"), { target: { value: "20/10/2026" } });
      expect(screen.getByText("más reciente")).toBeTruthy();
      expect(screen.queryByText("de octubre")).toBeNull();
    });

    it("la fecha y el texto se combinan", async () => {
      render(NotesSection, { api: notesApi(dated()) });
      await waitFor(() => expect(screen.getByText("más reciente")).toBeTruthy());
      await fireEvent.click(screen.getByRole("button", { name: "Filtrar por fecha" }));
      await typeDate("Desde la fecha", "2026-10-01");
      await fireEvent.input(screen.getByLabelText("Buscar notas"), { target: { value: "reciente" } });
      expect(screen.getByText("más reciente")).toBeTruthy();
      expect(screen.queryByText("de octubre")).toBeNull();
    });

    it("sin coincidencias lo dice", async () => {
      render(NotesSection, { api: notesApi(dated()) });
      await waitFor(() => expect(screen.getByText("más reciente")).toBeTruthy());
      await fireEvent.click(screen.getByRole("button", { name: "Filtrar por fecha" }));
      await typeDate("Desde la fecha", "2027-01-01");
      expect(screen.getByText("Ninguna nota coincide con la búsqueda.")).toBeTruthy();
    });
  });

  it("al borrar una nota se desvanece (clase leaving) y después sale de la lista", async () => {
    const day = (d: number) => new Date(2026, 9, d, 12, 0).getTime();
    const { container } = render(NotesSection, { api: notesApi([note("1", "primera", day(10)), note("2", "segunda", day(11))]) });
    await waitFor(() => expect(screen.getByText("segunda")).toBeTruthy());
    const li = screen.getByText("primera").closest("li")!;
    await fireEvent.click(li.querySelector(".danger")!);
    await fireEvent.click(li.querySelector(".danger")!);
    expect(li.classList.contains("leaving")).toBe(true);
    expect(screen.getByText("segunda")).toBeTruthy();
    await waitFor(() => expect(screen.queryByText("primera")).toBeNull());
    expect(container.querySelectorAll("li.note").length).toBe(1);
  });

  it("un error se muestra sin romper nada", async () => {
    render(NotesSection, { api: notesApi([], { add: vi.fn().mockRejectedValue("la nota es demasiado larga") }) });
    await fireEvent.input(screen.getByLabelText("Nueva nota"), { target: { value: "x" } });
    await fireEvent.click(screen.getByRole("button", { name: "Agregar nota" }));
    await waitFor(() => expect(screen.getByText("la nota es demasiado larga")).toBeTruthy());
  });
});

describe("EngineWallpapers", () => {
  const library: EngineLibrary = {
    available: true,
    items: [
      { id: "C:/w/1", title: "Lluvia", kind: "scene", preview: "C:/w/1/p.gif" },
      { id: "C:/w/2", title: "Bosque", kind: "video", preview: null },
      { id: "C:/w/3", title: "Reloj", kind: "web", preview: null },
    ],
    current: [
      { monitor: 0, id: "C:/w/1" },
      { monitor: 1, id: "C:/w/2" },
    ],
  };

  it("lista los fondos, marca los puestos y aplica con un clic", async () => {
    const onApply = vi.fn().mockResolvedValue({ simulated: false, description: "Fondo «Reloj» enviado" });
    render(EngineWallpapers, { library, onApply });
    expect(screen.getByText("3 de 3")).toBeTruthy();
    expect(screen.getAllByTitle("Puesto ahora").length).toBe(2);
    await fireEvent.click(screen.getByLabelText("Poner Reloj"));
    await waitFor(() => expect(onApply).toHaveBeenCalledWith("C:/w/3", null));
    await waitFor(() => expect(screen.getByText("Fondo «Reloj» enviado")).toBeTruthy());
  });

  it("aplica solo al monitor elegido", async () => {
    const onApply = vi.fn().mockResolvedValue({ simulated: false, description: "ok" });
    render(EngineWallpapers, { library, onApply });
    await fireEvent.click(screen.getByRole("radio", { name: "Monitor 2" }));
    await fireEvent.click(screen.getByLabelText("Poner Reloj"));
    await waitFor(() => expect(onApply).toHaveBeenCalledWith("C:/w/3", 1));
  });

  it("busca, filtra por tipo y por guardados", async () => {
    render(EngineWallpapers, { library, onApply: vi.fn() });
    await fireEvent.input(screen.getByLabelText("Buscar fondo"), { target: { value: "bosq" } });
    expect(screen.getByText("1 de 3")).toBeTruthy();
    await fireEvent.input(screen.getByLabelText("Buscar fondo"), { target: { value: "" } });
    await fireEvent.click(screen.getByRole("radio", { name: "Web" }));
    expect(screen.getByLabelText("Poner Reloj")).toBeTruthy();
    expect(screen.queryByLabelText("Poner Lluvia")).toBeNull();
    await fireEvent.click(screen.getByRole("radio", { name: "Todos" }));
    await fireEvent.click(screen.getByLabelText("Guardar Lluvia"));
    await fireEvent.click(screen.getByRole("button", { name: /Guardados/ }));
    expect(screen.getByLabelText("Poner Lluvia")).toBeTruthy();
    expect(screen.queryByLabelText("Poner Reloj")).toBeNull();
  });

  it("muestra el error si Wallpaper Engine no responde", async () => {
    render(EngineWallpapers, { library, onApply: vi.fn().mockRejectedValue("no se pudo pedir el cambio") });
    await fireEvent.click(screen.getByLabelText("Poner Reloj"));
    await waitFor(() => expect(screen.getByText("no se pudo pedir el cambio")).toBeTruthy());
  });

  const many: EngineLibrary = {
    available: true,
    items: Array.from({ length: 30 }, (_, i) => ({ id: `C:/w/${i}`, title: `Fondo ${String(i).padStart(2, "0")}`, kind: "scene", preview: null })),
    current: [],
  };

  it("pagina de 18 en 18 (tres filas de seis) con botones Anterior y Siguiente", async () => {
    render(EngineWallpapers, { library: many, onApply: vi.fn() });
    expect(screen.getByText("Página 1 de 2")).toBeTruthy();
    expect(screen.getAllByLabelText(/^Poner /).length).toBe(18);
    expect((screen.getByRole("button", { name: /Anterior/ }) as HTMLButtonElement).disabled).toBe(true);
    await fireEvent.click(screen.getByRole("button", { name: /Siguiente/ }));
    expect(screen.getByText("Página 2 de 2")).toBeTruthy();
    expect(screen.getByLabelText("Poner Fondo 18")).toBeTruthy();
    expect(screen.queryByLabelText("Poner Fondo 00")).toBeNull();
    expect(screen.getAllByLabelText(/^Poner /).length).toBe(12);
    expect((screen.getByRole("button", { name: /Siguiente/ }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("buscar vuelve a la primera página y con pocos resultados no hay paginador", async () => {
    render(EngineWallpapers, { library: many, onApply: vi.fn() });
    await fireEvent.click(screen.getByRole("button", { name: /Siguiente/ }));
    await fireEvent.input(screen.getByLabelText("Buscar fondo"), { target: { value: "fondo 2" } });
    expect(screen.queryByText(/Página/)).toBeNull();
    expect(screen.getAllByLabelText(/^Poner /).length).toBe(12);
    await fireEvent.input(screen.getByLabelText("Buscar fondo"), { target: { value: "" } });
    expect(screen.getByText("Página 1 de 2")).toBeTruthy();
  });

  it("al cambiar de página las páginas se desplazan según hacia dónde vas y la copia que sale se retira", async () => {
    vi.useFakeTimers();
    try {
      const { container } = render(EngineWallpapers, { library: many, onApply: vi.fn() });
      await fireEvent.click(screen.getByRole("button", { name: /Siguiente/ }));
      // Voy a la derecha: la página que sale se va por la izquierda y la nueva entra por la derecha.
      const leaving = container.querySelector(".grid.leaving");
      expect(leaving?.classList.contains("to-left")).toBe(true);
      expect(leaving?.querySelectorAll("button").length).toBe(0); // la copia no tiene botones
      expect(container.querySelector(".grid.from-right")).toBeTruthy();
      await vi.advanceTimersByTimeAsync(500);
      expect(container.querySelector(".grid.leaving")).toBeNull();
      expect(container.querySelector(".grid.from-right")).toBeNull();
      // Y al volver atrás, al revés.
      await fireEvent.click(screen.getByRole("button", { name: /Anterior/ }));
      expect(container.querySelector(".grid.leaving")?.classList.contains("to-right")).toBe(true);
      expect(container.querySelector(".grid.from-left")).toBeTruthy();
      expect(screen.getAllByLabelText(/^Poner /).length).toBe(18);
      await vi.advanceTimersByTimeAsync(500);
    } finally {
      vi.useRealTimers();
    }
  });

  it("las flechas del teclado cambian de página", async () => {
    render(EngineWallpapers, { library: many, onApply: vi.fn() });
    await fireEvent.keyDown(screen.getByRole("group", { name: "Fondos" }), { key: "ArrowRight" });
    expect(screen.getByText("Página 2 de 2")).toBeTruthy();
    await fireEvent.keyDown(screen.getByRole("group", { name: "Fondos" }), { key: "ArrowLeft" });
    expect(screen.getByText("Página 1 de 2")).toBeTruthy();
  });
});
