import { invoke } from "@tauri-apps/api/core";

export interface WindowInfo {
  handle: number;
  exe: string;
  title: string;
}

export function listWindows(): Promise<WindowInfo[]> {
  return invoke<WindowInfo[]>("list_windows");
}
