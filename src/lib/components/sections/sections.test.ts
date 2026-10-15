import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { RadioState, SystemOverview } from "../../api/system";
import ConnectivitySection from "./ConnectivitySection.svelte";
import MediaSection from "./MediaSection.svelte";
import PowerSection from "./PowerSection.svelte";
import StatsSection from "./StatsSection.svelte";
import WallpaperSection from "./WallpaperSection.svelte";

afterEach(cleanup);

const overview: SystemOverview = {
  cpu_percent: 41.2,
  gpu_percent: null,
  ram_used: 8 * 1024 ** 3,
  ram_total: 16 * 1024 ** 3,
  disks: [{ name: "C:", used: 500 * 1024 ** 3, total: 1000 * 1024 ** 3 }],
  net_down: 2 * 1024 * 1024,
  net_up: 512 * 1024,
  uptime_secs: 2 * 3600 + 37 * 60,
  temperature_c: null,
  top_processes: [
    { name: "chrome.exe", cpu_percent: 12.5, ram_bytes: 1024 ** 3 },
    { name: "code.exe", cpu_percent: 4, ram_bytes: 512 * 1024 ** 2 },
  ],
};

describe("MediaSection", () => {
  const media = { title: "Canción", artist: "Artista", app: "Spotify", playing: true };

  it("muestra lo que suena y manda las órdenes", async () => {
    const onCommand = vi.fn();
    render(MediaSection, { media, volume: null, onCommand, onVolume: vi.fn(), onMute: vi.fn() });
    expect(screen.getByText("Canción")).toBeTruthy();
    expect(screen.getByText("Artista · Spotify")).toBeTruthy();
    await fireEvent.click(screen.getByLabelText("Pausar"));
    await fireEvent.click(screen.getByLabelText("Siguiente"));
    await fireEvent.click(screen.getByLabelText("Anterior"));
    expect(onCommand.mock.calls.map((c) => c[0])).toEqual(["play_pause", "next", "previous"]);
  });

  it("sin nada sonando los controles están desactivados", () => {
    render(MediaSection, { media: null, volume: null, onCommand: vi.fn(), onVolume: vi.fn(), onMute: vi.fn() });
    expect(screen.getByText("No suena nada.")).toBeTruthy();
    expect((screen.getByLabelText("Reproducir") as HTMLButtonElement).disabled).toBe(true);
  });

  it("volumen: deslizador y silencio", async () => {
    const onVolume = vi.fn();
    const onMute = vi.fn();
    render(MediaSection, { media: null, volume: { percent: 40, muted: false }, onCommand: vi.fn(), onVolume, onMute });
    await fireEvent.change(screen.getByLabelText("Volumen"), { target: { value: "65" } });
    expect(onVolume).toHaveBeenCalledWith(65);
    await fireEvent.click(screen.getByLabelText("Silenciar"));
    expect(onMute).toHaveBeenCalledWith(true);
  });
});

describe("StatsSection", () => {
  it("muestra CPU, GPU, RAM, disco, red, tiempo encendido y las apps que más consumen", () => {
    render(StatsSection, { overview });
    expect(screen.getByText("41%")).toBeTruthy();
    expect(screen.getByText("no disponible")).toBeTruthy();
    expect(screen.getByText("8.0 GB de 16.0 GB")).toBeTruthy();
    expect(screen.getByText("Disco C:")).toBeTruthy();
    expect(screen.getByText("↓ 2.0 MB/s · ↑ 512.0 KB/s")).toBeTruthy();
    expect(screen.getByText("2 h 37 min")).toBeTruthy();
    expect(screen.getByText("chrome.exe")).toBeTruthy();
    expect(screen.queryByText("Temperatura")).toBeNull();
  });

  it("muestra temperatura y GPU cuando existen", () => {
    render(StatsSection, { overview: { ...overview, gpu_percent: 13.5, temperature_c: 61.4 } });
    expect(screen.getByText("14%")).toBeTruthy();
    expect(screen.getByText("61 °C")).toBeTruthy();
  });

  it("sin datos todavía", () => {
    render(StatsSection, { overview: null });
    expect(screen.getByText("Leyendo…")).toBeTruthy();
  });
});

describe("PowerSection", () => {
  it("pide confirmación antes de ejecutar y muestra que fue simulado", async () => {
    const onRun = vi.fn().mockResolvedValue({ simulated: true, description: "Simulado: se habría pedido apagar el equipo" });
    render(PowerSection, { allowChanges: false, onRun });
    expect(screen.getByText("modo seguro: simulado")).toBeTruthy();
    await fireEvent.click(screen.getByText("Apagar"));
    expect(onRun).not.toHaveBeenCalled();
    expect(screen.getByText(/¿Apagar el equipo\?/)).toBeTruthy();
    await fireEvent.click(screen.getByText("Sí"));
    expect(onRun).toHaveBeenCalledWith("shutdown");
    await waitFor(() => expect(screen.getByText("Simulado: se habría pedido apagar el equipo")).toBeTruthy());
  });

  it("cancelar no ejecuta nada", async () => {
    const onRun = vi.fn();
    render(PowerSection, { allowChanges: false, onRun });
    await fireEvent.click(screen.getByText("Reiniciar"));
    await fireEvent.click(screen.getByText("Cancelar"));
    expect(onRun).not.toHaveBeenCalled();
    expect(screen.getByText("Hibernar")).toBeTruthy();
  });

  it("las cuatro acciones existen y con permiso no muestra el aviso de modo seguro", async () => {
    const onRun = vi.fn().mockResolvedValue({ simulated: false, description: "Pedido: suspender el equipo" });
    render(PowerSection, { allowChanges: true, onRun });
    expect(screen.queryByText("modo seguro: simulado")).toBeNull();
    for (const label of ["Apagar", "Suspender", "Reiniciar", "Hibernar"]) expect(screen.getByText(label)).toBeTruthy();
    await fireEvent.click(screen.getByText("Suspender"));
    await fireEvent.click(screen.getByText("Sí"));
    expect(onRun).toHaveBeenCalledWith("suspend");
  });

  it("muestra el error si la acción falla", async () => {
    render(PowerSection, { allowChanges: true, onRun: vi.fn().mockRejectedValue("Windows rechazó la suspensión") });
    await fireEvent.click(screen.getByText("Suspender"));
    await fireEvent.click(screen.getByText("Sí"));
    await waitFor(() => expect(screen.getByText("Windows rechazó la suspensión")).toBeTruthy());
  });
});

describe("ConnectivitySection", () => {
  const radios: RadioState[] = [
    { kind: "wifi", available: true, on: true },
    { kind: "bluetooth", available: true, on: false },
    { kind: "hotspot", available: false, on: false },
  ];

  it("alterna Wi-Fi y Bluetooth; lo no disponible está desactivado", async () => {
    const onSet = vi.fn().mockResolvedValue({ simulated: true, description: "Simulado: ..." });
    render(ConnectivitySection, { radios, allowChanges: false, onSet });
    await fireEvent.click(screen.getByText("Wi-Fi: sí"));
    expect(onSet).toHaveBeenCalledWith("wifi", false);
    await fireEvent.click(screen.getByText("Bluetooth: no"));
    expect(onSet).toHaveBeenCalledWith("bluetooth", true);
    expect((screen.getByText("Compartir red: no disponible") as HTMLButtonElement).disabled).toBe(true);
    await waitFor(() => expect(screen.getByText("Simulado: ...")).toBeTruthy());
  });
});

describe("WallpaperSection", () => {
  it("aplica la imagen elegida", async () => {
    const onSet = vi.fn().mockResolvedValue({ simulated: false, description: "Fondo cambiado" });
    render(WallpaperSection, { list: { dir: "C:\\fondos", images: ["a.jpg", "b.png"], current: null }, onSet });
    await fireEvent.click(screen.getByText("b.png"));
    expect(onSet).toHaveBeenCalledWith("b.png");
    await waitFor(() => expect(screen.getByText("Fondo cambiado")).toBeTruthy());
  });

  it("sin imágenes explica dónde ponerlas", () => {
    render(WallpaperSection, { list: { dir: "C:\\fondos", images: [], current: null }, onSet: vi.fn() });
    expect(screen.getByText("C:\\fondos")).toBeTruthy();
  });
});
