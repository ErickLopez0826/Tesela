import { cleanup, render } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { InstalledModule, ModuleEventPayload } from "../api/modules";

let emit: (e: ModuleEventPayload) => void = () => {};
const unlisten = vi.fn();

vi.mock("../api/modules", async () => {
  const actual = await vi.importActual<typeof import("../api/modules")>("../api/modules");
  return {
    ...actual,
    moduleUrl: (id: string) => `about:blank#${id}`,
    modulesApi: {
      ...actual.modulesApi,
      onEvent: async (cb: (e: ModuleEventPayload) => void) => {
        emit = cb;
        return unlisten;
      },
    },
  };
});

import ModuleFrame from "./ModuleFrame.svelte";
import { MODULE_ACTION_EVENT } from "../api/modules";

const mod: InstalledModule = {
  manifest: {
    id: "hello",
    name: "Hola",
    version: "1.0.0",
    author: "x",
    description: "d",
    min_core_version: "0.1.0",
    permissions: ["windows"],
    network_hosts: [],
    dependencies: [],
    entry: "index.html",
    ui: { panel: { title: "Hola", height: 100 }, bar: null },
    actions: [],
    shortcuts: [],
  },
  granted: ["windows"],
  enabled: true,
};

function frameOf(container: HTMLElement) {
  const iframe = container.querySelector("iframe")!;
  const posted = vi.fn();
  Object.defineProperty(iframe.contentWindow, "postMessage", { value: posted, configurable: true });
  return { iframe, posted };
}

const message = (iframe: HTMLIFrameElement, data: unknown, source: Window | null = iframe.contentWindow) =>
  window.dispatchEvent(new MessageEvent("message", { data, source }));

beforeEach(() => unlisten.mockClear());
afterEach(cleanup);

describe("ModuleFrame", () => {
  it("el iframe va aislado: sandbox solo con scripts, sin same-origin", () => {
    const { container } = render(ModuleFrame, { module: mod, callCore: vi.fn() });
    const iframe = container.querySelector("iframe")!;
    expect(iframe.getAttribute("sandbox")).toBe("allow-scripts");
    expect(iframe.getAttribute("sandbox")).not.toContain("same-origin");
    expect(iframe.getAttribute("src")).toBe("about:blank#hello");
  });

  it("atiende las peticiones del módulo y responde al iframe correcto", async () => {
    const callCore = vi.fn().mockResolvedValue({ number: 3 });
    const { container } = render(ModuleFrame, { module: mod, callCore });
    const { iframe, posted } = frameOf(container);
    message(iframe, { wm: 1, id: 5, method: "windows.active_workspace", params: null });
    await vi.waitFor(() => expect(posted).toHaveBeenCalled());
    expect(callCore).toHaveBeenCalledWith("hello", "windows.active_workspace", null);
    expect(posted).toHaveBeenCalledWith({ wm: 1, id: 5, result: { number: 3 } }, "*");
  });

  it("ignora mensajes que no vienen de su iframe", async () => {
    const callCore = vi.fn().mockResolvedValue(1);
    const { container } = render(ModuleFrame, { module: mod, callCore });
    const { iframe, posted } = frameOf(container);
    message(iframe, { wm: 1, id: 1, method: "power.run", params: { action: "shutdown" } }, window);
    message(iframe, { wm: 1, id: 2, method: "power.run", params: {} }, null);
    await new Promise((r) => setTimeout(r, 30));
    expect(callCore).not.toHaveBeenCalled();
    expect(posted).not.toHaveBeenCalled();
  });

  it("devuelve al módulo el rechazo del núcleo", async () => {
    const callCore = vi.fn().mockRejectedValue("permission_denied: nope");
    const { container } = render(ModuleFrame, { module: mod, callCore });
    const { iframe, posted } = frameOf(container);
    message(iframe, { wm: 1, id: 9, method: "power.run", params: { action: "shutdown" } });
    await vi.waitFor(() => expect(posted).toHaveBeenCalledWith({ wm: 1, id: 9, error: "permission_denied: nope" }, "*"));
  });

  it("reenvía solo los eventos de su módulo y las acciones dirigidas a él", async () => {
    const { container } = render(ModuleFrame, { module: mod, callCore: vi.fn() });
    const { posted } = frameOf(container);
    await vi.waitFor(() => expect(emit).not.toBe(undefined));
    await new Promise((r) => setTimeout(r, 10));
    emit({ module: "hello", event: "workspace_changed", payload: { number: 2 } });
    emit({ module: "otro", event: "workspace_changed", payload: { number: 9 } });
    expect(posted).toHaveBeenCalledTimes(1);
    expect(posted).toHaveBeenCalledWith({ wm: 1, event: "workspace_changed", payload: { number: 2 } }, "*");
    window.dispatchEvent(new CustomEvent(MODULE_ACTION_EVENT, { detail: { module: "hello", action: "saludar" } }));
    window.dispatchEvent(new CustomEvent(MODULE_ACTION_EVENT, { detail: { module: "otro", action: "x" } }));
    expect(posted).toHaveBeenCalledTimes(2);
    expect(posted).toHaveBeenLastCalledWith({ wm: 1, action: "saludar" }, "*");
  });

  it("al destruirse deja de escuchar", async () => {
    const callCore = vi.fn().mockResolvedValue(1);
    const { container, unmount } = render(ModuleFrame, { module: mod, callCore });
    const { iframe } = frameOf(container);
    await new Promise((r) => setTimeout(r, 10));
    unmount();
    expect(unlisten).toHaveBeenCalled();
    message(iframe, { wm: 1, id: 1, method: "core.version", params: null });
    await new Promise((r) => setTimeout(r, 20));
    expect(callCore).not.toHaveBeenCalled();
  });
});
