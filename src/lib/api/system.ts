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
}

export interface DiskUsage {
  name: string;
  used: number;
  total: number;
}

export interface SystemOverview {
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
  mediaCommand: (command: MediaCommandName) => invoke<ActionOutcome>("media_command", { command }),
  volumeGet: () => invoke<VolumeState>("volume_get"),
  volumeSet: (percent: number) => invoke<ActionOutcome>("volume_set", { percent }),
  volumeMute: (muted: boolean) => invoke<ActionOutcome>("volume_mute", { muted }),
  radioStates: () => invoke<RadioState[]>("radio_states"),
  radioSet: (kind: RadioKindName, on: boolean) => invoke<ActionOutcome>("radio_set", { kind, on }),
  powerRun: (action: PowerActionName) => invoke<ActionOutcome>("power_run", { action }),
  wallpapersList: () => invoke<WallpaperList>("wallpapers_list"),
  wallpaperSet: (name: string) => invoke<ActionOutcome>("wallpaper_set", { name }),
};
