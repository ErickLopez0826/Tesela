import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
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

describe("manifiesto del módulo de utilidad", () => {
  it("pide solo lo que necesita", () => {
    expect([...speedManifest.permissions].sort()).toEqual(["files", "speedtest"]);
    expect(speedManifest.ui.panel.title).toBeTruthy();
    expect(speedManifest.actions.length).toBeGreaterThan(0);
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
