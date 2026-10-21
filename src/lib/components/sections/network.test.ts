import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AudioApi, NetworkApi, RadioState } from "../../api/system";
import { audioApi } from "../../api/system";
import ConnectivitySection from "./ConnectivitySection.svelte";

afterEach(cleanup);

const done = { simulated: false, description: "Hecho" };
const radios: RadioState[] = [
  { kind: "wifi", available: true, on: true },
  { kind: "bluetooth", available: true, on: true },
  { kind: "hotspot", available: true, on: false },
];

function api(over: Partial<NetworkApi> = {}): NetworkApi {
  return {
    wifiNetworks: vi.fn().mockResolvedValue([
      { ssid: "Casa", signal: 100, secured: true, connected: true },
      { ssid: "Vecino", signal: 50, secured: true, connected: false },
      { ssid: "Cafe", signal: 25, secured: false, connected: false },
    ]),
    wifiConnect: vi.fn().mockResolvedValue(done),
    wifiDisconnect: vi.fn().mockResolvedValue(done),
    bluetoothDevices: vi.fn().mockResolvedValue([{ id: "1", name: "Audífonos", connected: true }, { id: "2", name: "Ratón", connected: false }]),
    hotspotInfo: vi.fn().mockResolvedValue({ ssid: "MiPC", passphrase: "12345678", clients: [{ name: "Móvil", mac: "aa:bb" }], max_clients: 8 }),
    hotspotConfigure: vi.fn().mockResolvedValue(done),
    openSettings: vi.fn().mockResolvedValue(undefined),
    ...over,
  };
}

describe("detalles de conectividad", () => {
  it("sin la API de red no hay botones de detalle", () => {
    render(ConnectivitySection, { radios, allowChanges: true, onSet: vi.fn() });
    expect(screen.queryByLabelText("Detalles de Wi-Fi")).toBeNull();
  });

  it("Wi-Fi: lista las redes y conecta a una protegida con contraseña", async () => {
    const network = api();
    render(ConnectivitySection, { radios, allowChanges: true, onSet: vi.fn(), network });
    await fireEvent.click(screen.getByLabelText("Detalles de Wi-Fi"));
    await waitFor(() => expect(screen.getByText("Vecino")).toBeTruthy());
    expect(screen.getByText("conectada")).toBeTruthy();
    await fireEvent.click(screen.getByText("Vecino"));
    await fireEvent.input(screen.getByLabelText("Contraseña de Vecino"), { target: { value: "secreta123" } });
    await fireEvent.click(screen.getByRole("button", { name: "Conectar" }));
    await waitFor(() => expect(network.wifiConnect).toHaveBeenCalledWith("Vecino", "secreta123"));
  });

  it("Wi-Fi: una red abierta se conecta sin contraseña y la actual se desconecta", async () => {
    const network = api();
    render(ConnectivitySection, { radios, allowChanges: true, onSet: vi.fn(), network });
    await fireEvent.click(screen.getByLabelText("Detalles de Wi-Fi"));
    await waitFor(() => expect(screen.getByText("Cafe")).toBeTruthy());
    await fireEvent.click(screen.getByText("Cafe"));
    await fireEvent.click(screen.getByRole("button", { name: "Conectar" }));
    await waitFor(() => expect(network.wifiConnect).toHaveBeenCalledWith("Cafe", undefined));
    await fireEvent.click(screen.getByText("Casa"));
    await fireEvent.click(screen.getByRole("button", { name: "Desconectar" }));
    await waitFor(() => expect(network.wifiDisconnect).toHaveBeenCalled());
  });

  it("muestra el error si conectar falla", async () => {
    const network = api({ wifiConnect: vi.fn().mockRejectedValue("contraseña incorrecta") });
    render(ConnectivitySection, { radios, allowChanges: true, onSet: vi.fn(), network });
    await fireEvent.click(screen.getByLabelText("Detalles de Wi-Fi"));
    await waitFor(() => expect(screen.getByText("Vecino")).toBeTruthy());
    await fireEvent.click(screen.getByText("Vecino"));
    await fireEvent.click(screen.getByRole("button", { name: "Conectar" }));
    await waitFor(() => expect(screen.getByText("contraseña incorrecta")).toBeTruthy());
  });

  it("Bluetooth: muestra los dispositivos y abre los ajustes del sistema", async () => {
    const network = api();
    render(ConnectivitySection, { radios, allowChanges: true, onSet: vi.fn(), network });
    await fireEvent.click(screen.getByLabelText("Detalles de Bluetooth"));
    await waitFor(() => expect(screen.getByText("Audífonos")).toBeTruthy());
    expect(screen.getByText("conectado")).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Abrir ajustes de Bluetooth" }));
    expect(network.openSettings).toHaveBeenCalledWith("bluetooth");
  });

  it("Compartir red: muestra quién está conectado y guarda el nombre y la contraseña", async () => {
    const network = api();
    render(ConnectivitySection, { radios, allowChanges: true, onSet: vi.fn(), network });
    await fireEvent.click(screen.getByLabelText("Detalles de Compartir red"));
    await waitFor(() => expect(screen.getByText("Móvil")).toBeTruthy());
    expect(screen.getByText("1 de 8 conectados")).toBeTruthy();
    await fireEvent.input(screen.getByLabelText("Nombre de la red compartida"), { target: { value: "Otra" } });
    await fireEvent.click(screen.getByRole("button", { name: "Guardar" }));
    await waitFor(() => expect(network.hotspotConfigure).toHaveBeenCalledWith("Otra", "12345678"));
  });
});

describe("ficha de audio en Conectividad", () => {
  const outputs = [
    { id: "a", name: "Altavoces", default: false },
    { id: "b", name: "Auriculares", default: true },
  ];
  const draw = (audio: AudioApi, onAudioChanged = vi.fn()) => render(ConnectivitySection, { radios, allowChanges: true, onSet: vi.fn(), audio, onAudioChanged });

  it("es una ficha como Wi-Fi o Bluetooth: muestra la salida actual y al abrirla lista las demás", async () => {
    const audio: AudioApi = { outputs: vi.fn().mockResolvedValue(outputs), setOutput: vi.fn().mockResolvedValue(done), openSettings: vi.fn() };
    const onChanged = vi.fn();
    const { container } = draw(audio, onChanged);
    await waitFor(() => expect(screen.getByText("Auriculares")).toBeTruthy());
    expect(container.querySelectorAll(".tile").length).toBe(4); // Wi-Fi, Bluetooth, Compartir red y Audio
    expect(screen.queryByRole("radio", { name: /Altavoces/ })).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: /^Audio/ }));
    expect(screen.getByRole("radio", { name: /Auriculares/ }).getAttribute("aria-checked")).toBe("true");
    await fireEvent.click(screen.getByRole("radio", { name: /Altavoces/ }));
    await waitFor(() => expect(audio.setOutput).toHaveBeenCalledWith("a"));
    await waitFor(() => expect(onChanged).toHaveBeenCalled());
  });

  it("elegir la salida que ya está en uso no hace nada", async () => {
    const audio: AudioApi = { outputs: vi.fn().mockResolvedValue(outputs), setOutput: vi.fn(), openSettings: vi.fn() };
    draw(audio);
    await waitFor(() => expect(screen.getByText("Auriculares")).toBeTruthy());
    await fireEvent.click(screen.getByRole("button", { name: "Salidas de audio" }));
    await fireEvent.click(screen.getByRole("radio", { name: /Auriculares/ }));
    expect(audio.setOutput).not.toHaveBeenCalled();
  });

  it("con muchas salidas aparecen todas, dentro de una lista con scroll", async () => {
    const many = Array.from({ length: 7 }, (_, i) => ({ id: `d${i}`, name: `Salida ${i}`, default: i === 0 }));
    const audio: AudioApi = { outputs: vi.fn().mockResolvedValue(many), setOutput: vi.fn(), openSettings: vi.fn() };
    const { container } = draw(audio);
    await waitFor(() => expect(screen.getAllByText("Salida 0").length).toBeGreaterThan(0));
    await fireEvent.click(screen.getByRole("button", { name: /^Audio/ }));
    expect(screen.getAllByRole("radio").length).toBe(7);
    expect(container.querySelector(".list")).toBeTruthy();
  });
});

describe("salida de audio dentro de Conectividad", () => {
  it("el adaptador real tiene los métodos que espera AudioApi", () => {
    expect(typeof audioApi.outputs).toBe("function");
    expect(typeof audioApi.setOutput).toBe("function");
    expect(typeof audioApi.openSettings).toBe("function");
  });

  it("Conectividad muestra la ficha de audio y deja elegir otra salida", async () => {
    const audio: AudioApi = {
      outputs: vi.fn().mockResolvedValue([{ id: "a", name: "Altavoces", default: true }, { id: "b", name: "Auriculares", default: false }]),
      setOutput: vi.fn().mockResolvedValue(done),
      openSettings: vi.fn(),
    };
    render(ConnectivitySection, { radios, allowChanges: true, onSet: vi.fn(), audio });
    await waitFor(() => expect(screen.getByText("Altavoces")).toBeTruthy());
    await fireEvent.click(screen.getByRole("button", { name: /^Audio/ }));
    await fireEvent.click(screen.getByRole("radio", { name: /Auriculares/ }));
    await waitFor(() => expect(audio.setOutput).toHaveBeenCalledWith("b"));
  });

  it("sin audio no aparece el botón", () => {
    render(ConnectivitySection, { radios, allowChanges: true, onSet: vi.fn() });
    expect(screen.queryByRole("button", { name: /^Audio/ })).toBeNull();
  });
});

describe("listas de conectividad con muchas opciones", () => {
  it("aparecen todas las redes, en una lista con scroll", async () => {
    const networks = Array.from({ length: 8 }, (_, i) => ({ ssid: `Red ${i}`, signal: 50, secured: true, connected: i === 0 }));
    const { container } = render(ConnectivitySection, { radios, allowChanges: true, onSet: vi.fn(), network: api({ wifiNetworks: vi.fn().mockResolvedValue(networks) }) });
    await fireEvent.click(screen.getByLabelText("Detalles de Wi-Fi"));
    await waitFor(() => expect(screen.getByText("Red 7")).toBeTruthy());
    expect(container.querySelectorAll(".list li").length).toBe(8);
    expect(container.querySelector(".list")).toBeTruthy();
  });
});

describe("icono de Wi-Fi según la señal", () => {
  /** Arcos del icono de Wi-Fi: [apagados, encendidos]. */
  function arcs() {
    const paths = [...document.querySelector(".tile .ico svg")!.querySelectorAll("path.arc")];
    return { lit: paths.filter((p) => !p.classList.contains("dim")).length, dim: paths.filter((p) => p.classList.contains("dim")).length };
  }
  const wifi = (signal?: number | null, on = true): RadioState[] => [{ kind: "wifi", available: true, on, ...(signal === undefined ? {} : { signal }) }];
  const tile = () => document.querySelector(".tile") as HTMLElement;

  it("con buena señal se ven los tres arcos y con menos se van apagando", () => {
    const seen = [5, 4, 3, 2, 1].map((bars) => {
      const { unmount } = render(ConnectivitySection, { radios: wifi(bars), allowChanges: true, onSet: vi.fn() });
      const lit = arcs().lit;
      unmount();
      return lit;
    });
    expect(seen).toEqual([3, 2, 2, 1, 1]);
  });

  it("encendido pero sin conexión solo queda el punto", () => {
    render(ConnectivitySection, { radios: wifi(null), allowChanges: true, onSet: vi.fn() });
    expect(arcs()).toEqual({ lit: 0, dim: 3 });
    expect(tile().title).toBe("Sin conexión");
  });

  it("si el núcleo no informa de la señal el icono se ve completo y sin tooltip", () => {
    render(ConnectivitySection, { radios: wifi(undefined), allowChanges: true, onSet: vi.fn() });
    expect(arcs()).toEqual({ lit: 3, dim: 0 });
    expect(tile().title).toBe("");
  });

  it("el tooltip dice cuántas barras hay y el icono sigue a la señal cuando cambia", async () => {
    const { rerender } = render(ConnectivitySection, { radios: wifi(4), allowChanges: true, onSet: vi.fn() });
    expect(tile().title).toBe("Señal: 4 de 5");
    expect(arcs().lit).toBe(2);
    await rerender({ radios: wifi(5), allowChanges: true, onSet: vi.fn() });
    expect(arcs().lit).toBe(3);
  });

  it("con el Wi-Fi apagado no se muestra señal y las demás conexiones conservan su icono", () => {
    render(ConnectivitySection, { radios: [...wifi(4, false), { kind: "bluetooth", available: true, on: true }], allowChanges: true, onSet: vi.fn() });
    expect(arcs().lit).toBe(3);
    expect(tile().title).toBe("");
    expect(document.querySelectorAll(".tile")[1].querySelectorAll("path.arc").length).toBe(0);
  });
});

describe("internet por cable (Ethernet)", () => {
  const tile = () => document.querySelector(".tile") as HTMLElement;
  const hasCable = () => tile().querySelectorAll(".ico svg path").length === 5 && !tile().querySelector("path.arc");

  it("un equipo de escritorio sin antena Wi-Fi, conectado por cable, muestra Ethernet con el icono de cable y activo", () => {
    render(ConnectivitySection, { radios: [{ kind: "wifi", available: false, on: false, ethernet: true }], allowChanges: true, onSet: vi.fn() });
    expect(tile().textContent).toContain("Ethernet");
    expect(tile().textContent).toContain("conectado");
    expect(tile().textContent).not.toContain("no disponible");
    expect(tile().classList.contains("on")).toBe(true);
    expect(hasCable()).toBe(true);
    expect(tile().title).toContain("cable");
    expect((tile() as HTMLButtonElement).disabled).toBe(true);
  });

  it("con antena Wi-Fi pero saliendo por cable, el icono es el cable (no un Wi-Fi vacío) y la ficha sigue encendiendo el Wi-Fi", async () => {
    const onSet = vi.fn().mockResolvedValue({ simulated: false, description: "ok" });
    render(ConnectivitySection, { radios: [{ kind: "wifi", available: true, on: true, signal: null, ethernet: true }], allowChanges: true, onSet });
    expect(tile().textContent).toContain("Wi-Fi");
    expect(hasCable()).toBe(true);
    await fireEvent.click(tile());
    expect(onSet).toHaveBeenCalledWith("wifi", false);
  });

  it("sin cable no cambia nada: el icono de Wi-Fi con sus arcos", () => {
    render(ConnectivitySection, { radios: [{ kind: "wifi", available: true, on: true, signal: 4, ethernet: false }], allowChanges: true, onSet: vi.fn() });
    expect(tile().querySelectorAll("path.arc").length).toBe(3);
    expect(tile().textContent).toContain("Wi-Fi");
  });
});
