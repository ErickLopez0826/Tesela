import { writable } from "svelte/store";
import * as api from "../api/exclusions";
import type { ExclusionRule } from "../api/exclusions";

export interface ExclusionsApi {
  list: () => Promise<ExclusionRule[]>;
  add: (exe: string, title: string | null) => Promise<ExclusionRule[]>;
  remove: (exe: string, title: string | null) => Promise<ExclusionRule[]>;
}

const tauriApi: ExclusionsApi = {
  list: api.listExclusions,
  add: api.addExclusion,
  remove: api.removeExclusion,
};

export interface ExclusionsState {
  rules: ExclusionRule[];
  error: string | null;
}

/** Misma normalización que el núcleo: minúsculas y sin ".exe". */
export function normalizeExe(exe: string): string {
  const e = exe.trim().toLowerCase();
  return e.endsWith(".exe") ? e.slice(0, -4) : e;
}

/** ¿Hay una exclusión que cubra todo el ejecutable (sin criterio de título)? */
export function isFullyExcluded(rules: ExclusionRule[], exe: string): boolean {
  const n = normalizeExe(exe);
  return rules.some((r) => normalizeExe(r.exe) === n && !r.title_contains);
}

export function createExclusionsStore(backend: ExclusionsApi = tauriApi) {
  const { subscribe, update } = writable<ExclusionsState>({ rules: [], error: null });

  async function run(op: () => Promise<ExclusionRule[]>) {
    try {
      const rules = await op();
      update(() => ({ rules, error: null }));
    } catch (e) {
      update((s) => ({ ...s, error: String(e) }));
    }
  }

  return {
    subscribe,
    refresh: () => run(backend.list),
    add: (exe: string, title: string | null = null) => run(() => backend.add(exe, title)),
    remove: (exe: string, title: string | null = null) => run(() => backend.remove(exe, title)),
  };
}

export const exclusionsStore = createExclusionsStore();
