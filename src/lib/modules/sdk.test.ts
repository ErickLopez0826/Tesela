import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import sdkCode from "../../../src-tauri/module-sdk.js?raw";

// El SDK corre dentro del iframe del módulo: aquí se prueba en jsdom con un "padre" falso.
type Wm = {
  call: (m: string, p?: unknown) => Promise<unknown>;
  on: (e: string, h: (p: unknown) => void) => Promise<unknown>;
  off: (e: string) => Promise<unknown>;
  onAction: (h: (id: string) => void) => void;
  onChannel: (c: string, h: (m: { channel: string; text: string }) => void) => Promise<unknown>;
  ready: Promise<unknown>;
};

let sent: { wm: number; id: number; method: string; params: unknown }[];
let fakeParent: { postMessage: (m: never, o: string) => void };
const realParent = Object.getOwnPropertyDescriptor(window, "parent");

function loadSdk(): Wm {
  new Function(sdkCode)();
  const wm = (window as unknown as { wm: Wm }).wm;
  // `ready` queda pendiente en la mayoría de pruebas; que su vencimiento no cuente como error sin atender.
  wm.ready.catch(() => {});
  return wm;
}

const fromParent = (data: unknown) => window.dispatchEvent(new MessageEvent("message", { data, source: fakeParent as unknown as Window }));

beforeEach(() => {
  sent = [];
  fakeParent = { postMessage: (m) => void sent.push(m as never) };
  Object.defineProperty(window, "parent", { value: fakeParent, configurable: true });
  delete (window as unknown as { wm?: Wm }).wm;
});

afterEach(() => {
  vi.useRealTimers();
  if (realParent) Object.defineProperty(window, "parent", realParent);
});

describe("SDK de módulos", () => {
  it("call envía la petición al núcleo y resuelve con la respuesta", async () => {
    const wm = loadSdk();
    sent.length = 0; // descarta el `core.version` de `ready`
    const p = wm.call("windows.active_workspace");
    expect(sent).toHaveLength(1);
    expect(sent[0]).toMatchObject({ wm: 1, method: "windows.active_workspace", params: null });
    fromParent({ wm: 1, id: sent[0].id, result: { number: 2 } });
    await expect(p).resolves.toEqual({ number: 2 });
  });

  it("call rechaza con el error del núcleo", async () => {
    const wm = loadSdk();
    sent.length = 0;
    const p = wm.call("power.run", { action: "shutdown" });
    fromParent({ wm: 1, id: sent[0].id, error: "permission_denied: nope" });
    await expect(p).rejects.toThrow("permission_denied: nope");
  });

  it("ready se resuelve con la versión del núcleo", async () => {
    const wm = loadSdk();
    expect(sent[0].method).toBe("core.version");
    fromParent({ wm: 1, id: sent[0].id, result: { version: "0.1.0" } });
    await expect(wm.ready).resolves.toEqual({ version: "0.1.0" });
  });

  it("on pide la suscripción y entrega los eventos al manejador", async () => {
    const wm = loadSdk();
    sent.length = 0;
    const handler = vi.fn();
    const sub = wm.on("workspace_changed", handler);
    expect(sent[0]).toMatchObject({ method: "events.subscribe", params: { event: "workspace_changed" } });
    fromParent({ wm: 1, id: sent[0].id, result: true });
    await sub;
    fromParent({ wm: 1, event: "workspace_changed", payload: { number: 4 } });
    expect(handler).toHaveBeenCalledWith({ number: 4 });
    fromParent({ wm: 1, event: "otro_evento", payload: 1 });
    expect(handler).toHaveBeenCalledTimes(1);
  });

  it("onChannel se suscribe y entrega solo los mensajes de ese canal", async () => {
    const wm = loadSdk();
    sent.length = 0;
    const handler = vi.fn();
    const sub = wm.onChannel("pareja", handler);
    expect(sent[0]).toMatchObject({ method: "channels.subscribe", params: { channel: "pareja" } });
    fromParent({ wm: 1, id: sent[0].id, result: true });
    await sub;
    fromParent({ wm: 1, event: "channel_message", payload: { channel: "pareja", text: "hola" } });
    fromParent({ wm: 1, event: "channel_message", payload: { channel: "otro", text: "no" } });
    expect(handler).toHaveBeenCalledTimes(1);
    expect(handler).toHaveBeenCalledWith({ channel: "pareja", text: "hola" });
  });

  it("onAction recibe las acciones del usuario", () => {
    const wm = loadSdk();
    const handler = vi.fn();
    wm.onAction(handler);
    fromParent({ wm: 1, action: "saludar" });
    expect(handler).toHaveBeenCalledWith("saludar");
  });

  it("ignora mensajes que no vienen del padre o no son del protocolo", async () => {
    const wm = loadSdk();
    sent.length = 0;
    const handler = vi.fn();
    wm.onAction(handler);
    window.dispatchEvent(new MessageEvent("message", { data: { wm: 1, action: "x" }, source: window }));
    fromParent({ action: "y" });
    fromParent(null);
    expect(handler).not.toHaveBeenCalled();
  });

  it("si el núcleo no responde, la llamada vence", async () => {
    vi.useFakeTimers();
    const wm = loadSdk();
    const p = wm.call("windows.focused");
    const assertion = expect(p).rejects.toThrow("no respondió");
    await vi.advanceTimersByTimeAsync(16000);
    await assertion;
  });

  it("no depende de Tauri ni expone nada más que `wm`", () => {
    const wm = loadSdk();
    expect(Object.keys(wm).sort()).toEqual(["call", "off", "on", "onAction", "onChannel", "ready"]);
    expect(sdkCode).not.toMatch(/__TAURI|invoke\(|eval\(/);
  });
});
