import { describe, expect, it, vi } from "vitest";
import { actionMessage, createBridge, eventMessage, parseRequest } from "./bridge";

describe("parseRequest", () => {
  it("acepta peticiones bien formadas", () => {
    expect(parseRequest({ wm: 1, id: 3, method: "windows.focused", params: { a: 1 } })).toEqual({
      wm: 1,
      id: 3,
      method: "windows.focused",
      params: { a: 1 },
    });
    expect(parseRequest({ wm: 1, id: 1, method: "core.version" })?.params).toBeNull();
  });

  it("ignora todo lo demás", () => {
    for (const bad of [null, undefined, "x", 3, [], { id: 1, method: "a" }, { wm: 2, id: 1, method: "a" }, { wm: 1, id: "1", method: "a" }, { wm: 1, id: 1.5, method: "a" }, { wm: 1, id: 1, method: "" }, { wm: 1, id: 1, method: "x".repeat(65) }, { wm: 1, id: 1, method: 5 }]) {
      expect(parseRequest(bad)).toBeNull();
    }
  });
});

describe("createBridge", () => {
  it("reenvía al núcleo con el id de módulo que fijó el anfitrión, no el del mensaje", async () => {
    const call = vi.fn().mockResolvedValue({ number: 2 });
    const handle = createBridge("hello", call);
    // El módulo intenta colar otro id en el mensaje: se ignora.
    const reply = await handle({ wm: 1, id: 7, method: "windows.active_workspace", params: null, module: "otro" });
    expect(call).toHaveBeenCalledWith("hello", "windows.active_workspace", null);
    expect(reply).toEqual({ wm: 1, id: 7, result: { number: 2 } });
  });

  it("devuelve el error del núcleo (p. ej. permiso denegado) sin lanzar", async () => {
    const records: unknown[] = [];
    const handle = createBridge("hello", async () => Promise.reject("permission_denied: 'power.run' necesita el permiso 'power'"), (r) => records.push(r));
    const reply = await handle({ wm: 1, id: 1, method: "power.run", params: { action: "shutdown" } });
    expect(reply).toEqual({ wm: 1, id: 1, error: "permission_denied: 'power.run' necesita el permiso 'power'" });
    expect(records).toEqual([
      { module: "hello", method: "power.run", params: { action: "shutdown" }, ok: false, error: "permission_denied: 'power.run' necesita el permiso 'power'" },
    ]);
  });

  it("no responde a mensajes que no son del protocolo", async () => {
    const call = vi.fn();
    const handle = createBridge("hello", call);
    expect(await handle({ hola: "mundo" })).toBeNull();
    expect(call).not.toHaveBeenCalled();
  });

  it("un resultado indefinido se entrega como null", async () => {
    const handle = createBridge("hello", async () => undefined);
    expect(await handle({ wm: 1, id: 2, method: "notify.show", params: { text: "x" } })).toEqual({ wm: 1, id: 2, result: null });
  });
});

describe("mensajes hacia el módulo", () => {
  it("evento y acción", () => {
    expect(eventMessage("workspace_changed", { number: 2 })).toEqual({ wm: 1, event: "workspace_changed", payload: { number: 2 } });
    expect(actionMessage("saludar")).toEqual({ wm: 1, action: "saludar" });
  });
});
