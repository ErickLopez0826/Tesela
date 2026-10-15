import { invoke } from "@tauri-apps/api/core";

export interface ExclusionRule {
  exe: string;
  title_contains: string | null;
}

export function listExclusions(): Promise<ExclusionRule[]> {
  return invoke<ExclusionRule[]>("list_exclusions");
}

export function addExclusion(exe: string, titleContains: string | null): Promise<ExclusionRule[]> {
  return invoke<ExclusionRule[]>("add_exclusion", { exe, titleContains });
}

export function removeExclusion(exe: string, titleContains: string | null): Promise<ExclusionRule[]> {
  return invoke<ExclusionRule[]>("remove_exclusion", { exe, titleContains });
}
