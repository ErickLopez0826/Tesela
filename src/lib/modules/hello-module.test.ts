import { describe, expect, it, vi } from "vitest";
import code from "../../../modules/examples/hello/app.js?raw";
import manifest from "../../../modules/examples/hello/module.json";

// El módulo de ejemplo corre aquí contra una API `wm` falsa: lo que hace con ella es lo que haría en el panel.
type Start = (wm: unknown, doc: Document) => Promise<void>;
const start = new Function(`${code}\nreturn start;`)() as Start;

function makeDoc() {
  document.body.innerHTML = '<div id="out"></div>';
  return document;
}

function fakeWm(handlers: Record<string, (params?: unknown) => unknown> = {}) {
  const events = new Map<string, (p: unknown) => void>();
  let action: (id: string) => void = () => {};
  const files = new Map<string, string>();
  const calls: [string, unknown][] = [];
  const defaults: Record<string, (params?: unknown) => unknown> = {
    "files.read": (p) => {
      const c = files.get((p as { path: string }).path);
      return c === undefined ? null : { content: c };
    },
    "files.write": (p) => {
      files.set((p as { path: string }).path, (p as { content: string }).content);
      return true;
    },
    "windows.active_workspace": () => ({ number: 2 }),
    "windows.focused": () => ({ exe: "code.exe", title: "Editor" }),
    "notify.show": () => null,
  };
  const wm = {
    call: vi.fn(async (method: string, params?: unknown) => {
      calls.push([method, params]);
      const h = handlers[method] ?? defaults[method];
      if (!h) throw new Error(`unknown_method: ${method}`);
      return h(params);
    }),
    on: vi.fn(async (event: string, h: (p: unknown) => void) => void events.set(event, h)),
    onAction: vi.fn((h: (id: string) => void) => void (action = h)),
  };
  return { wm, files, calls, emit: (e: string, p: unknown) => events.get(e)?.(p), act: (id: string) => action(id) };
}

const text = () => document.getElementById("out")!.textContent;

describe("módulo de ejemplo «hello»", () => {
  it("su manifiesto pide exactamente lo que usa", () => {
    expect(manifest.permissions.sort()).toEqual(["files", "notifications", "windows"]);
    expect(manifest.actions.map((a) => a.id)).toEqual(["saludar"]);
    expect(manifest.shortcuts[0].action).toBe("saludar");
  });

  it("muestra workspace y ventana enfocada y cuenta sus aperturas", async () => {
    const f = fakeWm();
    await start(f.wm, makeDoc());
    expect(text()).toContain("Aperturas1");
    expect(text()).toContain("Workspace activo2");
    expect(text()).toContain("Ventana enfocadaEditor");
    expect(f.files.get("contador.txt")).toBe("1");
    document.body.innerHTML = "";
    await start(f.wm, makeDoc());
    expect(f.files.get("contador.txt")).toBe("2");
    expect(text()).toContain("Aperturas2");
  });

  it("se actualiza con los eventos del núcleo", async () => {
    const f = fakeWm();
    await start(f.wm, makeDoc());
    f.emit("workspace_changed", { number: 5 });
    expect(text()).toContain("Workspace activo5");
    f.emit("focus_changed", { exe: "x", title: "Navegador" });
    expect(text()).toContain("Ventana enfocadaNavegador");
    f.emit("focus_changed", null);
    expect(text()).toContain("Ventana enfocada—");
  });

  it("la acción «saludar» muestra un aviso", async () => {
    const f = fakeWm();
    await start(f.wm, makeDoc());
    f.act("otra");
    f.act("saludar");
    await vi.waitFor(() => expect(f.calls.filter((c) => c[0] === "notify.show")).toHaveLength(1));
    expect((f.calls.find((c) => c[0] === "notify.show")![1] as { text: string }).text).toContain("Aperturas: 1");
  });

  it("si el núcleo le niega un permiso, lo muestra y no se rompe", async () => {
    const denied = () => {
      throw new Error("permission_denied: 'windows.active_workspace' necesita el permiso 'windows', que no se concedió");
    };
    const f = fakeWm({ "windows.active_workspace": denied, "windows.focused": denied });
    await start(f.wm, makeDoc());
    expect(text()).toContain("permission_denied");
    expect(text()).toContain("Aperturas1");
  });
});
