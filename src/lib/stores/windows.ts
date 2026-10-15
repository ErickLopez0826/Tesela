import { writable } from "svelte/store";
import { listWindows, type WindowInfo } from "../api/windows";

export interface WindowsState {
  windows: WindowInfo[];
  loading: boolean;
  error: string | null;
}

export function createWindowsStore(fetchWindows: () => Promise<WindowInfo[]> = listWindows) {
  const { subscribe, update } = writable<WindowsState>({
    windows: [],
    loading: false,
    error: null,
  });

  async function refresh() {
    update((s) => ({ ...s, loading: true, error: null }));
    try {
      const windows = await fetchWindows();
      update((s) => ({ ...s, windows, loading: false }));
    } catch (e) {
      update((s) => ({ ...s, loading: false, error: String(e) }));
    }
  }

  return { subscribe, refresh };
}

export const windowsStore = createWindowsStore();
