import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import appsCode from "../../../modules/apps/app.js?raw";
import appsHtml from "../../../modules/apps/index.html?raw";
import appsManifest from "../../../modules/apps/module.json";
import clipCode from "../../../modules/clipboard/app.js?raw";
import clipHtml from "../../../modules/clipboard/index.html?raw";
import clipManifest from "../../../modules/clipboard/module.json";
import notesCode from "../../../modules/notes/app.js?raw";
import notesHtml from "../../../modules/notes/index.html?raw";
import notesManifest from "../../../modules/notes/module.json";
import speedCode from "../../../modules/speedtest/app.js?raw";
import speedHtml from "../../../modules/speedtest/index.html?raw";
import speedManifest from "../../../modules/speedtest/module.json";

// Los módulos corren aquí contra una API `wm` falsa con un sistema de archivos en memoria. Se evalúan los
// archivos reales del repositorio (nada que venga de fuera).
type Handler = (params: any) => unknown;
type Fake = ReturnType<typeof makeWm>;

function makeWm(handlers: Record<string, Handler> = {}) {
  const files = new Map<string, string>();
  const calls: [string, any][] = [];
  let action: (id: string) => void = () => {};
  const base: Record<string, Handler> = {
    "files.read": (p) => (files.has(p.path) ? { content: files.get(p.path) } : null),
    "files.write": (p) => (files.set(p.path, p.content), true),
    "files.delete": (p) => files.delete(p.path),
    "files.list": () => [...files.keys()].sort(),
    "files.dir": () => ({ path: "C:\\datos\\modulo" }),
  };
  const wm = {
    call: vi.fn(async (method: string, params?: any) => {
      calls.push([method, params]);
      const h = handlers[method] ?? base[method];
      if (!h) throw new Error(`unknown_method: ${method}`);
      return h(params);
    }),
    on: vi.fn(async () => true),
    onAction: vi.fn((h: (id: string) => void) => void (action = h)),
  };
  return { wm, files, calls, act: (id: string) => action(id), called: (m: string) => calls.filter((c) => c[0] === m) };
}

function mount(html: string) {
  const body = new DOMParser().parseFromString(html, "text/html").body;
  document.body.innerHTML = body.innerHTML; // los <script src> no se ejecutan; los módulos se cargan aparte
}

function load<T>(code: string, names: string): T {
  return new Function(`${code}\nreturn { ${names} };`)() as T;
}

const q = <E extends HTMLElement>(sel: string) => document.querySelector(sel) as E;
const tick = (ms = 0) => vi.advanceTimersByTimeAsync(ms);

beforeEach(() => vi.useFakeTimers({ toFake: ["setInterval", "clearInterval", "setTimeout", "clearTimeout"] }));
afterEach(() => {
  vi.useRealTimers();
  document.body.innerHTML = "";
});

// ---------------------------------------------------------------- manifiestos

describe("manifiestos de los módulos de utilidad", () => {
  it("cada uno pide solo lo que necesita", () => {
    expect(notesManifest.permissions).toEqual(["files"]);
    expect([...appsManifest.permissions].sort()).toEqual(["files", "launch", "windows"]);
    expect([...clipManifest.permissions].sort()).toEqual(["clipboard", "files"]);
    expect([...speedManifest.permissions].sort()).toEqual(["files", "speedtest"]);
    for (const m of [notesManifest, appsManifest, clipManifest, speedManifest]) {
      expect(m.ui.panel.title).toBeTruthy();
      expect(m.actions.length).toBeGreaterThan(0);
    }
    expect(clipManifest.ui).toHaveProperty("bar");
  });
});

// ---------------------------------------------------------------- notas

describe("módulo Notas", () => {
  const api = load<{
    start: (wm: unknown, doc: Document) => Promise<void>;
    safeNoteName: (raw: string, existing: string[]) => string;
    isNoteFile: (p: string) => boolean;
    renderMarkdown: (t: string, d: Document) => DocumentFragment;
  }>(notesCode, "start, safeNoteName, isNoteFile, renderMarkdown");

  it("nombres de archivo seguros y sin repetir", () => {
    expect(api.safeNoteName("Mi nota", [])).toBe("Mi nota.md");
    expect(api.safeNoteName("../../etc/passwd", [])).toBe("etcpasswd.md");
    expect(api.safeNoteName("a\\b:c*d?.txt", [])).toBe("abcd.md");
    expect(api.safeNoteName("", [])).toBe("Nota.md");
    expect(api.safeNoteName("Idea", ["Idea.md", "Idea 2.md"])).toBe("Idea 3.md");
    expect(api.safeNoteName("ñandú día", [])).toBe("ñandú día.md");
    expect(api.safeNoteName("x".repeat(200), []).length).toBeLessThanOrEqual(63);
  });

  it("reconoce archivos de notas", () => {
    expect(api.isNoteFile("notas/a.md")).toBe(true);
    expect(api.isNoteFile("notas/b.TXT")).toBe(true);
    expect(api.isNoteFile("notas/c.exe")).toBe(false);
    expect(api.isNoteFile("otra/a.md")).toBe(false);
    expect(api.isNoteFile("fijadas.json")).toBe(false);
  });

  it("Markdown: títulos, énfasis, código, listas y bloques", () => {
    const frag = api.renderMarkdown("# Título\n\nHola **negrita** y *cursiva* con `código`\n\n- uno\n- dos\n\n```\nlinea1\nlinea2\n```", document);
    const host = document.createElement("div");
    host.appendChild(frag);
    expect(host.querySelector("h1")?.textContent).toBe("Título");
    expect(host.querySelector("strong")?.textContent).toBe("negrita");
    expect(host.querySelector("em")?.textContent).toBe("cursiva");
    expect(host.querySelector("p code")?.textContent).toBe("código");
    expect([...host.querySelectorAll("li")].map((l) => l.textContent)).toEqual(["uno", "dos"]);
    expect(host.querySelector("pre code")?.textContent).toBe("linea1\nlinea2");
  });

  it("Markdown es seguro: el HTML escrito en una nota es texto, nunca elementos", () => {
    const evil = '<img src=x onerror="alert(1)"> <script>alert(2)</script>\n# <b>x</b>';
    const host = document.createElement("div");
    host.appendChild(api.renderMarkdown(evil, document));
    expect(host.querySelector("img, script, b")).toBeNull();
    expect(host.textContent).toContain("<img src=x");
    expect(host.textContent).toContain("<script>alert(2)</script>");
  });

  it("primera vez: crea y abre una nota de bienvenida", async () => {
    mount(notesHtml);
    const f = makeWm();
    await api.start(f.wm, document);
    expect([...f.files.keys()]).toEqual(["notas/Bienvenida.md"]);
    expect(q<HTMLTextAreaElement>("#editor").value).toContain("# Bienvenida");
    expect([...document.querySelectorAll("#list option")].map((o) => o.textContent)).toEqual(["Bienvenida"]);
    expect(q("#status").textContent).toContain("C:\\datos\\modulo\\notas");
  });

  it("se guarda solo, medio segundo después de escribir", async () => {
    mount(notesHtml);
    const f = makeWm();
    await api.start(f.wm, document);
    const ed = q<HTMLTextAreaElement>("#editor");
    ed.value = "# Cambiado";
    ed.dispatchEvent(new Event("input"));
    ed.value = "# Cambiado otra vez";
    ed.dispatchEvent(new Event("input"));
    await tick(400);
    expect(f.files.get("notas/Bienvenida.md")).not.toBe("# Cambiado otra vez");
    await tick(200);
    expect(f.files.get("notas/Bienvenida.md")).toBe("# Cambiado otra vez");
    expect(f.called("files.write").filter((c) => c[1].content === "# Cambiado").length).toBe(0);
  });

  it("la acción «nueva» crea notas sin pisar las existentes y se pueden borrar", async () => {
    mount(notesHtml);
    const f = makeWm();
    await api.start(f.wm, document);
    f.act("nueva");
    await tick(10);
    f.act("nueva");
    await tick(10);
    expect([...f.files.keys()].sort()).toEqual(["notas/Bienvenida.md", "notas/Nueva nota 2.md", "notas/Nueva nota.md"]);
    q<HTMLButtonElement>("#delete").click();
    await tick(10);
    expect(f.files.has("notas/Nueva nota 2.md")).toBe(false);
    expect(f.files.size).toBe(2);
  });

  it("alterna entre editar y ver el formato", async () => {
    mount(notesHtml);
    await api.start(makeWm().wm, document);
    q<HTMLButtonElement>("#toggle").click();
    expect(q("#preview").hidden).toBe(false);
    expect(q("#editor").hidden).toBe(true);
    expect(q("#preview h1")?.textContent).toBe("Bienvenida");
    expect(q("#toggle").textContent).toBe("Editar");
  });

  it("muestra el error si el núcleo niega el permiso", async () => {
    mount(notesHtml);
    const deny = () => {
      throw new Error("permission_denied: 'files.list' necesita el permiso 'files'");
    };
    await api.start(makeWm({ "files.list": deny, "files.dir": deny }).wm, document);
    expect(q("#status").textContent).toContain("permission_denied");
    expect(q("#status").className).toBe("error");
  });
});

// ---------------------------------------------------------------- apps

describe("módulo Apps abiertas y fijadas", () => {
  const api = load<{
    start: (wm: unknown, doc: Document) => Promise<void>;
    pin: (l: { name: string; path: string }[], e: { name?: string; path: string }) => { name: string; path: string }[];
    unpin: (l: { name: string; path: string }[], p: string) => { name: string; path: string }[];
    parsePinned: (t: string) => { name: string; path: string }[];
    openWindowFor: (o: { path: string; handle: number }[], p: { path: string }) => { handle: number } | null;
    baseName: (p: string) => string;
  }>(appsCode, "start, pin, unpin, parsePinned, openWindowFor, baseName");

  const open = [
    { handle: 11, exe: "Code.exe", path: "C:\\Apps\\Code.exe", title: "Editor" },
    { handle: 12, exe: "chrome.exe", path: "C:\\Apps\\chrome.exe", title: "Web" },
  ];

  it("fijar sin duplicados (ignora mayúsculas y barras) y quitar", () => {
    let l = api.pin([], { path: "C:\\Apps\\Code.exe" });
    expect(l).toEqual([{ name: "Code", path: "C:\\Apps\\Code.exe" }]);
    expect(api.pin(l, { path: "c:/apps/code.EXE" })).toBe(l);
    l = api.pin(l, { path: "C:\\Apps\\b.lnk", name: "B" });
    expect(api.unpin(l, "C:/APPS/CODE.exe").map((e) => e.name)).toEqual(["B"]);
  });

  it("lee fijadas descartando lo inválido", () => {
    expect(api.parsePinned("no es json")).toEqual([]);
    expect(api.parsePinned('{"a":1}')).toEqual([]);
    expect(api.parsePinned('[{"path":"C:\\\\a.exe"},{"path":"C:\\\\a.txt"},{"x":1},null]').map((e) => e.path)).toEqual(["C:\\a.exe"]);
    expect(api.baseName("C:\\Archivos\\Discord.exe")).toBe("Discord");
  });

  it("encuentra la ventana abierta de una app fijada", () => {
    expect(api.openWindowFor(open, { path: "c:\\apps\\CODE.exe" })?.handle).toBe(11);
    expect(api.openWindowFor(open, { path: "C:\\otra.exe" })).toBeNull();
  });

  it("muestra las abiertas, salta a una y la fija (se guarda en fijadas.json)", async () => {
    mount(appsHtml);
    const f = makeWm({ "apps.list_open": () => open, "apps.focus": () => true });
    await api.start(f.wm, document);
    const rows = [...document.querySelectorAll(".item")];
    expect(rows.map((r) => r.querySelector(".name")?.firstChild?.textContent)).toEqual(["Editor", "Web"]);
    [...rows[0].querySelectorAll("button")].find((b) => b.textContent === "Ir")!.click();
    await tick(0);
    expect(f.called("apps.focus")).toEqual([["apps.focus", { handle: 11 }]]);
    [...rows[1].querySelectorAll("button")].find((b) => b.textContent === "Fijar")!.click();
    await tick(0);
    expect(JSON.parse(f.files.get("fijadas.json")!)).toEqual([{ name: "chrome", path: "C:\\Apps\\chrome.exe" }]);
  });

  it("las fijadas se abren, o se saltan a ellas si ya están abiertas; y se quitan", async () => {
    mount(appsHtml);
    const f = makeWm({ "apps.list_open": () => [open[0]], "apps.launch": () => true, "apps.focus": () => true });
    f.files.set("fijadas.json", JSON.stringify([{ name: "Code", path: "C:\\Apps\\Code.exe" }, { name: "Discord", path: "C:\\Apps\\Discord.exe" }]));
    await api.start(f.wm, document);
    q<HTMLButtonElement>("#tab-pinned").click();
    const rows = [...document.querySelectorAll(".item")];
    expect(rows).toHaveLength(2);
    expect(rows[0].textContent).toContain("abierta");
    [...rows[0].querySelectorAll("button")].find((b) => b.textContent === "Ir")!.click();
    [...rows[1].querySelectorAll("button")].find((b) => b.textContent === "Abrir")!.click();
    await tick(0);
    expect(f.called("apps.focus")[0][1]).toEqual({ handle: 11 });
    expect(f.called("apps.launch")[0][1]).toEqual({ path: "C:\\Apps\\Discord.exe" });
    [...rows[1].querySelectorAll("button")].find((b) => b.textContent === "Quitar")!.click();
    await tick(0);
    expect(JSON.parse(f.files.get("fijadas.json")!).map((e: { name: string }) => e.name)).toEqual(["Code"]);
  });

  it("fijar escribiendo la ruta valida lo escrito; la acción alterna las listas", async () => {
    mount(appsHtml);
    const f = makeWm({ "apps.list_open": () => [] });
    await api.start(f.wm, document);
    expect(q("#tab-open").className).toBe("active");
    f.act("alternar");
    expect(q("#tab-pinned").className).toBe("active");
    expect(q("#add").hidden).toBe(false);
    const input = q<HTMLInputElement>("#path");
    input.value = "cmd /c calc";
    q<HTMLFormElement>("#add").dispatchEvent(new Event("submit", { cancelable: true }));
    expect(q("#status").className).toBe("error");
    expect(f.files.has("fijadas.json")).toBe(false);
    input.value = "D:\\Juegos\\Steam\\steam.exe";
    q<HTMLFormElement>("#add").dispatchEvent(new Event("submit", { cancelable: true }));
    await tick(0);
    expect(JSON.parse(f.files.get("fijadas.json")!)[0]).toEqual({ name: "steam", path: "D:\\Juegos\\Steam\\steam.exe" });
    f.act("alternar");
    expect(q("#tab-open").className).toBe("active");
  });

  it("las abiertas se refrescan en tiempo real", async () => {
    mount(appsHtml);
    let current = [open[0]];
    const f = makeWm({ "apps.list_open": () => current });
    await api.start(f.wm, document);
    expect(document.querySelectorAll(".item")).toHaveLength(1);
    current = [open[0], open[1]];
    await tick(2100);
    expect(document.querySelectorAll(".item")).toHaveLength(2);
  });
});

// ---------------------------------------------------------------- portapapeles

describe("módulo Historial del portapapeles", () => {
  const api = load<{
    start: (wm: unknown, doc: Document) => Promise<void>;
    addEntry: (h: string[], t: string) => string[];
    parseHistory: (t: string) => string[];
    preview: (t: string) => string;
  }>(clipCode, "start, addEntry, parseHistory, preview");

  it("reglas del historial", () => {
    expect(api.addEntry([], "a")).toEqual(["a"]);
    expect(api.addEntry(["b", "a"], "a")).toEqual(["a", "b"]);
    expect(api.addEntry(["a"], "   ")).toEqual(["a"]);
    expect(api.addEntry(["a"], "x".repeat(5001))).toEqual(["a"]);
    const many = Array.from({ length: 60 }, (_, i) => `t${i}`);
    expect(api.addEntry(many.slice(0, 50), "nuevo")).toHaveLength(50);
    expect(api.addEntry(many.slice(0, 50), "nuevo")[0]).toBe("nuevo");
    expect(api.parseHistory('["a",3,null,"b"]')).toEqual(["a", "b"]);
    expect(api.parseHistory("roto")).toEqual([]);
    expect(api.preview("uno\n\n  dos")).toBe("uno dos");
    expect(api.preview("x".repeat(200)).length).toBe(88);
  });

  it("registra lo que se copia, sin duplicados, y lo guarda en disco", async () => {
    mount(clipHtml);
    let clip: string | null = "primero";
    const f = makeWm({ "clipboard.read": () => (clip === null ? null : { text: clip }), "clipboard.write": () => true });
    await api.start(f.wm, document);
    expect(JSON.parse(f.files.get("historial.json")!)).toEqual(["primero"]);
    clip = "segundo";
    await tick(1000);
    clip = "segundo";
    await tick(1000);
    clip = null; // portapapeles privado/vacío: no se guarda nada
    await tick(1000);
    expect(JSON.parse(f.files.get("historial.json")!)).toEqual(["segundo", "primero"]);
    expect(q("#count").textContent).toBe("2");
    expect([...document.querySelectorAll(".entry")].map((b) => b.textContent)).toEqual(["segundo", "primero"]);
  });

  it("un clic vuelve a copiar la entrada y no la registra de nuevo", async () => {
    mount(clipHtml);
    let clip = "uno";
    const f = makeWm({ "clipboard.read": () => ({ text: clip }), "clipboard.write": (p: { text: string }) => ((clip = p.text), true) });
    await api.start(f.wm, document);
    clip = "dos";
    await tick(1000);
    (document.querySelectorAll(".entry")[1] as HTMLButtonElement).click(); // "uno"
    await tick(1500);
    expect(f.called("clipboard.write")).toEqual([["clipboard.write", { text: "uno" }]]);
    expect(JSON.parse(f.files.get("historial.json")!)).toEqual(["dos", "uno"]);
  });

  it("la barra y el panel comparten el archivo sin pisarse", async () => {
    mount(clipHtml);
    const f = makeWm({ "clipboard.read": () => ({ text: "desde la barra" }) });
    f.files.set("historial.json", JSON.stringify(["escrito por el panel"]));
    await api.start(f.wm, document);
    expect(JSON.parse(f.files.get("historial.json")!)).toEqual(["desde la barra", "escrito por el panel"]);
  });

  it("«limpiar» borra el historial", async () => {
    mount(clipHtml);
    const f = makeWm({ "clipboard.read": () => ({ text: "x" }) });
    await api.start(f.wm, document);
    f.act("limpiar");
    await tick(0);
    expect(f.files.get("historial.json")).toBe("[]");
    expect(q("#count").textContent).toBe("0");
  });

  it("ignora el texto enorme (p. ej. un archivo pegado)", async () => {
    mount(clipHtml);
    const f = makeWm({ "clipboard.read": () => ({ text: "x".repeat(6000) }) });
    await api.start(f.wm, document);
    expect(f.files.has("historial.json")).toBe(false);
  });
});

// ---------------------------------------------------------------- prueba de velocidad

describe("módulo Prueba de velocidad", () => {
  const api = load<{
    start: (wm: unknown, doc: Document, now?: () => string) => Promise<void>;
    addResult: (h: unknown[], r: Record<string, unknown>, when: string) => { download_mbps: number; when: string }[];
    parseHistory: (t: string) => unknown[];
    fmt: (n: number) => string;
  }>(speedCode, "start, addResult, parseHistory, fmt");

  const result = { download_mbps: 250.04, upload_mbps: 40, ping_ms: 11.5, server: "Ejemplo Net (CDMX)", isp: "Mi ISP", simulated: false };

  it("formato e historial acotado", () => {
    expect(api.fmt(250.04)).toBe("250.0");
    expect(api.fmt(0.96)).toBe("1.0");
    let h: unknown[] = [];
    for (let i = 0; i < 15; i++) h = api.addResult(h, { ...result, download_mbps: i }, `t${i}`);
    expect(h).toHaveLength(10);
    expect((h[0] as { download_mbps: number }).download_mbps).toBe(14);
    expect(api.parseHistory('[{"download_mbps":1},{"x":2},3]')).toHaveLength(1);
  });

  it("inicia, muestra «midiendo», consulta cada segundo y al terminar muestra el resultado y lo guarda", async () => {
    mount(speedHtml);
    let calls = 0;
    const f = makeWm({
      "speedtest.start": () => true,
      "speedtest.status": () => (++calls < 3 ? { state: "running" } : { state: "done", result }),
    });
    await api.start(f.wm, document, () => "2026-10-14T22:30:00");
    q<HTMLButtonElement>("#run").click();
    await tick(0);
    expect(q<HTMLButtonElement>("#run").disabled).toBe(true);
    expect(q("#status").textContent).toContain("Midiendo");
    await tick(2200);
    expect(q<HTMLButtonElement>("#run").disabled).toBe(false);
    expect(q("#result").textContent).toContain("↓ 250.0");
    expect(q("#result").textContent).toContain("↑ 40.0");
    expect(q("#result").textContent).toContain("Ejemplo Net (CDMX) · Mi ISP");
    expect(q("#result .badge")).toBeNull();
    const saved = JSON.parse(f.files.get("historial.json")!);
    expect(saved).toHaveLength(1);
    expect(saved[0].when).toBe("2026-10-14T22:30:00");
    // Sin más consultas una vez terminada.
    const before = f.called("speedtest.status").length;
    await tick(3000);
    expect(f.called("speedtest.status").length).toBe(before);
  });

  it("avisa cuando los datos son de ejemplo (CLI de Ookla sin instalar)", async () => {
    mount(speedHtml);
    const f = makeWm({ "speedtest.start": () => true, "speedtest.status": () => ({ state: "done", result: { ...result, simulated: true } }) });
    await api.start(f.wm, document);
    f.act("iniciar");
    await tick(10);
    expect(q("#result .badge")?.textContent).toContain("CLI de Ookla no está instalado");
  });

  it("muestra el fallo y permite reintentar", async () => {
    mount(speedHtml);
    const f = makeWm({ "speedtest.start": () => true, "speedtest.status": () => ({ state: "failed", error: "sin conexión" }) });
    await api.start(f.wm, document);
    q<HTMLButtonElement>("#run").click();
    await tick(10);
    expect(q("#status").textContent).toBe("La prueba falló: sin conexión");
    expect(q("#status").className).toBe("error");
    expect(q<HTMLButtonElement>("#run").disabled).toBe(false);
  });

  it("si ya había una en curso la sigue en vez de fallar", async () => {
    mount(speedHtml);
    let started = false;
    const f = makeWm({
      "speedtest.start": () => {
        started = true;
        throw new Error("bad_params: ya hay una prueba en curso");
      },
      "speedtest.status": () => (started ? { state: "done", result } : { state: "idle" }),
    });
    await api.start(f.wm, document);
    q<HTMLButtonElement>("#run").click();
    await tick(10);
    expect(q("#result").textContent).toContain("↓ 250.0");
  });

  it("al abrir recupera la última medición guardada", async () => {
    mount(speedHtml);
    const f = makeWm({ "speedtest.status": () => ({ state: "idle" }) });
    f.files.set("historial.json", JSON.stringify([{ download_mbps: 100, upload_mbps: 20, ping_ms: 9, simulated: false, when: "2026-10-01T10:00:00" }]));
    await api.start(f.wm, document);
    await tick(0);
    expect(q("#result").textContent).toContain("↓ 100.0");
    expect(q("#result").textContent).toContain("2026-10-01 10:00");
  });
});
