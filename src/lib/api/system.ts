import { invoke } from "@tauri-apps/api/core";

export interface ActionOutcome {
  /** `true`: no se tocó nada de verdad (modo seguro). */
  simulated: boolean;
  description: string;
}

export interface ProcessUsage {
  name: string;
  cpu_percent: number;
  ram_bytes: number;
  /** Uso de la gráfica (0-100). */
  gpu_percent: number;
  /** Lectura + escritura en disco, bytes por segundo. */
  disk_bytes_per_sec: number;
}

export interface DiskUsage {
  name: string;
  used: number;
  total: number;
}

export interface HardwareSpecs {
  cpu_name: string;
  cpu_cores: number;
  cpu_threads: number;
  /** Frecuencia en MHz (0 si no se conoce). */
  cpu_mhz: number;
  gpu_name: string | null;
  /** Memoria de vídeo dedicada en bytes (0 en gráficas integradas). */
  gpu_vram: number;
}

export interface SystemOverview {
  hardware: HardwareSpecs;
  cpu_percent: number;
  gpu_percent: number | null;
  ram_used: number;
  ram_total: number;
  disks: DiskUsage[];
  net_down: number;
  net_up: number;
  uptime_secs: number;
  temperature_c: number | null;
  top_processes: ProcessUsage[];
}

export interface MediaInfo {
  title: string;
  artist: string;
  app: string;
  playing: boolean;
}

export type MediaCommandName = "play_pause" | "next" | "previous";

export interface VolumeState {
  percent: number;
  muted: boolean;
}

export type RadioKindName = "wifi" | "bluetooth" | "hotspot";

export interface RadioState {
  kind: RadioKindName;
  available: boolean;
  on: boolean;
  /** Solo el Wi-Fi: barras de señal (0 a 5) de la red conectada, o `null` si está encendido sin conexión. */
  signal?: number | null;
}

export interface WifiNetwork {
  ssid: string;
  /** 0-100 */
  signal: number;
  secured: boolean;
  connected: boolean;
}

export interface BluetoothDevice {
  id: string;
  name: string;
  connected: boolean;
}

export interface HotspotClient {
  name: string;
  mac: string;
}

export interface HotspotInfo {
  ssid: string;
  passphrase: string;
  clients: HotspotClient[];
  max_clients: number;
}

export interface AudioDevice {
  id: string;
  name: string;
  default: boolean;
}

export type SettingsPage = "bluetooth" | "wifi" | "hotspot" | "sound";

/** Lo que necesitan los detalles de conectividad (se pasa entero para poder probarlos sin Tauri). */
export interface NetworkApi {
  wifiNetworks: () => Promise<WifiNetwork[]>;
  wifiConnect: (ssid: string, password?: string) => Promise<ActionOutcome>;
  wifiDisconnect: () => Promise<ActionOutcome>;
  bluetoothDevices: () => Promise<BluetoothDevice[]>;
  hotspotInfo: () => Promise<HotspotInfo | null>;
  hotspotConfigure: (ssid: string, passphrase: string) => Promise<ActionOutcome>;
  openSettings: (page: SettingsPage) => Promise<void>;
}

export interface AudioApi {
  outputs: () => Promise<AudioDevice[]>;
  setOutput: (id: string) => Promise<ActionOutcome>;
  openSettings: (page: SettingsPage) => Promise<void>;
}

export interface EngineWallpaper {
  /** Carpeta del fondo: es lo único que se acepta para aplicarlo. */
  id: string;
  title: string;
  /** scene, video, web… (puede ser vacío) */
  kind: string;
  /** Ruta absoluta de la previsualización. */
  preview: string | null;
}

export interface EngineCurrent {
  monitor: number;
  id: string;
}

export interface EngineLibrary {
  available: boolean;
  items: EngineWallpaper[];
  current: EngineCurrent[];
}

export interface Note {
  id: string;
  text: string;
  /** ms desde 1970 */
  created: number;
  updated: number;
}

/** Lo que necesita la pestaña de notas (se pasa entero para poder probarla sin Tauri). */
export interface NotesApi {
  list: () => Promise<Note[]>;
  add: (text: string) => Promise<Note>;
  update: (id: string, text: string) => Promise<Note>;
  remove: (id: string) => Promise<void>;
  /** Guarda un archivo en Documentos/Tesela/Notas y devuelve su ruta. */
  exportFile: (name: string, content: string) => Promise<string>;
  openFolder: () => Promise<void>;
  clipboardRead: () => Promise<string | null>;
  clipboardWrite: (text: string) => Promise<void>;
}

export type PowerActionName = "shutdown" | "suspend" | "restart" | "hibernate";

export interface WallpaperList {
  dir: string;
  images: string[];
  current: string | null;
}

export interface SystemMode {
  allow_changes: boolean;
}

export const systemApi = {
  mode: () => invoke<SystemMode>("system_mode"),
  overview: () => invoke<SystemOverview>("overview"),
  mediaNow: () => invoke<MediaInfo | null>("media_now"),
  mediaThumbnail: () => invoke<string | null>("media_thumbnail"),
  mediaCommand: (command: MediaCommandName) => invoke<ActionOutcome>("media_command", { command }),
  volumeGet: () => invoke<VolumeState>("volume_get"),
  volumeSet: (percent: number) => invoke<ActionOutcome>("volume_set", { percent }),
  volumeMute: (muted: boolean) => invoke<ActionOutcome>("volume_mute", { muted }),
  radioStates: () => invoke<RadioState[]>("radio_states"),
  radioSet: (kind: RadioKindName, on: boolean) => invoke<ActionOutcome>("radio_set", { kind, on }),
  wifiNetworks: () => invoke<WifiNetwork[]>("wifi_networks"),
  wifiConnect: (ssid: string, password?: string) => invoke<ActionOutcome>("wifi_connect", { ssid, password: password ?? null }),
  wifiDisconnect: () => invoke<ActionOutcome>("wifi_disconnect"),
  bluetoothDevices: () => invoke<BluetoothDevice[]>("bluetooth_devices"),
  hotspotInfo: () => invoke<HotspotInfo | null>("hotspot_info"),
  hotspotConfigure: (ssid: string, passphrase: string) => invoke<ActionOutcome>("hotspot_configure", { ssid, passphrase }),
  audioOutputs: () => invoke<AudioDevice[]>("audio_outputs"),
  audioSetOutput: (id: string) => invoke<ActionOutcome>("audio_set_output", { id }),
  openSettings: (page: SettingsPage) => invoke<void>("open_windows_settings", { page }),
  engineLibrary: () => invoke<EngineLibrary>("engine_library"),
  engineApply: (id: string, monitor: number | null) => invoke<ActionOutcome>("engine_apply", { id, monitor }),
  powerRun: (action: PowerActionName) => invoke<ActionOutcome>("power_run", { action }),
  wallpapersList: () => invoke<WallpaperList>("wallpapers_list"),
  wallpaperSet: (name: string) => invoke<ActionOutcome>("wallpaper_set", { name }),
};

/** Adaptador real de la salida de audio: los nombres de `AudioApi` no son los de `systemApi`. */
export const audioApi: AudioApi = {
  outputs: systemApi.audioOutputs,
  setOutput: systemApi.audioSetOutput,
  openSettings: systemApi.openSettings,
};

export const notesApi: NotesApi = {
  list: () => invoke<Note[]>("notes_list"),
  add: (text) => invoke<Note>("notes_add", { text }),
  update: (id, text) => invoke<Note>("notes_update", { id, text }),
  remove: (id) => invoke<void>("notes_delete", { id }),
  exportFile: (name, content) => invoke<string>("notes_export", { name, content }),
  openFolder: () => invoke<void>("notes_open_folder"),
  clipboardRead: () => invoke<string | null>("clipboard_read"),
  clipboardWrite: (text) => invoke<void>("clipboard_write", { text }),
};
