import { invoke } from "@tauri-apps/api/core";

export interface GeneralStatus {
  version: string;
  managing: boolean;
  bar: boolean;
  autostart_enabled: boolean;
  autostart_command: string | null;
  config_dir: string;
  bundled_modules_dir: string | null;
  updater_compiled: boolean;
}

export interface UpdateInfo {
  version: string;
  notes: string | null;
}

export const generalApi = {
  status: () => invoke<GeneralStatus>("general_status"),
  setAutostart: (enabled: boolean) => invoke<GeneralStatus>("autostart_set", { enabled }),
  restart: (manage: boolean, bar: boolean) => invoke<void>("restart_app", { manage, bar }),
  checkUpdates: () => invoke<UpdateInfo | null>("updates_check"),
  installUpdate: () => invoke<void>("updates_install"),
};
