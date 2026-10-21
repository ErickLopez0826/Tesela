import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ClaudeApi, HooksStatus, UsageView } from "../api/claude";
import ClaudeSettings from "./ClaudeSettings.svelte";

afterEach(cleanup);

const off: HooksStatus = { installed: false, partial: false, settings_path: "C:/Users/x/.claude/settings.json", script_path: "C:/Users/x/AppData/Local/Tesela/claude/tesela-claude-hook.js", settings_ok: true };
const on: HooksStatus = { ...off, installed: true };

const usageOff: UsageView = { enabled: false, status: "off", windows: [], breakdown: [], fetched_at: null, problem: null, message: null };
const NOW = Math.floor(Date.now() / 1000);
const usageOn: UsageView = {
  enabled: true,
  status: "ok",
  windows: [
    { id: "five_hour", label: "5 h", percent: 35, resets_at: NOW + 8130, level: "ok" },
    { id: "seven_day", label: "Semana", percent: 24, resets_at: NOW + 4 * 86400 + 30, level: "ok" },
  ],
  breakdown: [{ name: "Claude Code", percent: 88 }],
  fetched_at: NOW - 120,
  problem: null,
  message: null,
};

function api(initial: HooksStatus = off, over: Partial<ClaudeApi> = {}): ClaudeApi {
  return {
    usageGet: vi.fn().mockResolvedValue(usageOff),
    usageSetEnabled: vi.fn().mockImplementation((enabled: boolean) => Promise.resolve(enabled ? usageOn : usageOff)),
    usageRefresh: vi.fn().mockResolvedValue(usageOn),
    hooksStatus: vi.fn().mockResolvedValue(initial),
    hooksInstall: vi.fn().mockResolvedValue(on),
    hooksUninstall: vi.fn().mockResolvedValue(off),
    ...over,
  };
}

describe("ClaudeSettings", () => {
  it("sin instalar ofrece instalar, y no toca nada hasta confirmar", async () => {
    const a = api();
    render(ClaudeSettings, { api: a });
    expect(await screen.findByText("No instalado")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Quitar" })).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "Instalar" }));
    // El paso de confirmación dice qué archivo cambia.
    expect(screen.getByText(/C:\/Users\/x\/\.claude\/settings\.json/)).toBeTruthy();
    expect(a.hooksInstall).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByRole("button", { name: "Confirmar instalación" }));
    await waitFor(() => expect(screen.getByText("Instalado")).toBeTruthy());
    expect(a.hooksInstall).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("button", { name: "Instalar" })).toBeNull();
  });

  it("cancelar no instala", async () => {
    const a = api();
    render(ClaudeSettings, { api: a });
    await fireEvent.click(await screen.findByRole("button", { name: "Instalar" }));
    await fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(a.hooksInstall).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Instalar" })).toBeTruthy();
  });

  it("instalado ofrece quitar, también en dos pasos", async () => {
    const a = api(on);
    render(ClaudeSettings, { api: a });
    expect(await screen.findByText("Instalado")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Instalar" })).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "Quitar" }));
    expect(a.hooksUninstall).not.toHaveBeenCalled();
    expect(screen.getByText(/Se quitan los hooks de Tesela/)).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Confirmar" }));
    await waitFor(() => expect(screen.getByText("No instalado")).toBeTruthy());
    expect(a.hooksUninstall).toHaveBeenCalledTimes(1);
  });

  it("a medias se puede reparar o quitar", async () => {
    render(ClaudeSettings, { api: api({ ...off, partial: true }) });
    expect(await screen.findByText("Instalado a medias")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Reparar" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Quitar" })).toBeTruthy();
  });

  it("si settings.json no es válido lo avisa y no deja instalar", async () => {
    render(ClaudeSettings, { api: api({ ...off, settings_ok: false }) });
    expect((await screen.findByRole("alert")).textContent).toContain("no es un JSON válido");
    expect((screen.getByRole("button", { name: "Instalar" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("si instalar falla enseña el motivo y sigue sin estar instalado", async () => {
    const a = api(off, { hooksInstall: vi.fn().mockRejectedValue("no se pudo escribir") });
    render(ClaudeSettings, { api: a });
    await fireEvent.click(await screen.findByRole("button", { name: "Instalar" }));
    await fireEvent.click(screen.getByRole("button", { name: "Confirmar instalación" }));
    expect((await screen.findByText(/no se pudo escribir/)).textContent).toContain("no se pudo escribir");
    expect(screen.getByText("No instalado")).toBeTruthy();
  });
});

describe("ClaudeSettings: uso de la suscripción", () => {
  const box = () => screen.getByRole("checkbox", { name: /Mostrar el uso de mi suscripción/ }) as HTMLInputElement;

  it("viene apagado y sin explicaciones de más", async () => {
    const a = api();
    render(ClaudeSettings, { api: a });
    await waitFor(() => expect(box().disabled).toBe(false));
    expect(box().checked).toBe(false);
    const text = document.body.textContent ?? "";
    expect(text).not.toContain(".credentials.json");
    expect(text).not.toContain("términos de");
    expect(a.usageSetEnabled).not.toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: "Actualizar ahora" })).toBeNull();
  });

  it("activarlo pide la lectura, enseña lo leído y deja actualizar a mano", async () => {
    const a = api();
    render(ClaudeSettings, { api: a });
    await waitFor(() => expect(box().disabled).toBe(false));
    await fireEvent.click(box());
    await waitFor(() => expect(a.usageSetEnabled).toHaveBeenCalledWith(true));
    expect(await screen.findByText(/5 h: 35% · reinicia en 2 h 15 min/)).toBeTruthy();
    expect(screen.getByText(/Esta semana: Claude Code 88%/)).toBeTruthy();
    expect(screen.getByText(/Última lectura hace 2 min/)).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Actualizar ahora" }));
    await waitFor(() => expect(a.usageRefresh).toHaveBeenCalledTimes(1));
  });

  it("apagarlo vuelve a pedir al núcleo que lo apague y oculta el detalle", async () => {
    const a = api(off, { usageGet: vi.fn().mockResolvedValue(usageOn) });
    render(ClaudeSettings, { api: a });
    await waitFor(() => expect(box().checked).toBe(true));
    await fireEvent.click(box());
    await waitFor(() => expect(a.usageSetEnabled).toHaveBeenCalledWith(false));
    await waitFor(() => expect(screen.queryByRole("button", { name: "Actualizar ahora" })).toBeNull());
  });

  it("si no hay sesión de Claude Code enseña el motivo como aviso", async () => {
    const signedOut: UsageView = { ...usageOff, enabled: true, status: "waiting", problem: "signed_out", message: "Claude Code no tiene sesión iniciada con tu suscripción en este equipo." };
    render(ClaudeSettings, { api: api(off, { usageGet: vi.fn().mockResolvedValue(signedOut) }) });
    expect((await screen.findByRole("alert")).textContent).toContain("no tiene sesión iniciada");
    expect(screen.queryByText("Leyendo…")).toBeNull();
  });

  it("con una lectura vieja la enseña junto al motivo, sin darla por mala", async () => {
    const stale: UsageView = { ...usageOn, status: "stale", problem: "offline", message: "Sin conexión con Claude." };
    render(ClaudeSettings, { api: api(off, { usageGet: vi.fn().mockResolvedValue(stale) }) });
    expect(await screen.findByText(/5 h: 35%/)).toBeTruthy();
    expect(screen.getByText("Sin conexión con Claude.")).toBeTruthy();
    expect(screen.queryByText(/Última lectura/)).toBeNull();
  });

  it("si activarlo falla enseña el motivo", async () => {
    const a = api(off, { usageSetEnabled: vi.fn().mockRejectedValue("no se pudo guardar") });
    render(ClaudeSettings, { api: a });
    await waitFor(() => expect(box().disabled).toBe(false));
    await fireEvent.click(box());
    expect((await screen.findByText(/no se pudo guardar/)).textContent).toContain("no se pudo guardar");
  });
});
