import { get } from "svelte/store";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { StateSnapshot } from "../api/state";
import {
  createStateStore,
  createStatsStore,
  formatClock,
  formatGb,
  formatPercent,
  ramPercent,
  workspacesFor,
} from "./wmState";

const state: StateSnapshot = {
  monitors: [
    { id: 1, primary: true, vertical: false, visible_workspace: 1, bar_visible: true },
    { id: 2, primary: false, vertical: true, visible_workspace: 5, bar_visible: true },
  ],
  workspaces: [
    { number: 1, monitor: 1, visible: true, active: true, window_count: 2 },
    { number: 2, monitor: 1, visible: false, active: false, window_count: 0 },
    { number: 3, monitor: 1, visible: false, active: false, window_count: 1 },
    { number: 5, monitor: 2, visible: true, active: false, window_count: 0 },
  ],
  focused: { exe: "code.exe", title: "Proyecto" },
  paused: false,
  mode: null,
};

afterEach(() => vi.useRealTimers());

describe("workspacesFor", () => {
  it("muestra los visibles o con ventanas de ese monitor", () => {
    expect(workspacesFor(state, 1).map((w) => w.number)).toEqual([1, 3]);
    expect(workspacesFor(state, 2).map((w) => w.number)).toEqual([5]);
    expect(workspacesFor(null, 1)).toEqual([]);
  });
});

describe("formato", () => {
  it("porcentajes acotados", () => {
    expect(formatPercent(42.4)).toBe("42%");
    expect(formatPercent(150)).toBe("100%");
    expect(formatPercent(-3)).toBe("0%");
  });

  it("RAM", () => {
    const stats = { cpu_percent: 1, ram_used: 8 * 1024 ** 3, ram_total: 16 * 1024 ** 3 };
    expect(ramPercent(stats)).toBe(50);
    expect(formatGb(stats.ram_used)).toBe("8.0 GB");
    expect(ramPercent({ ...stats, ram_total: 0 })).toBe(0);
  });

  it("reloj con ceros", () => {
    const d = new Date(2026, 0, 2, 3, 4, 5);
    expect(formatClock(d)).toBe("03:04");
    expect(formatClock(d, true)).toBe("03:04:05");
  });
});

describe("state store", () => {
  it("toma el estado inicial y luego las actualizaciones", async () => {
    let push: (s: StateSnapshot) => void = () => {};
    const store = createStateStore({
      get: async () => state,
      listen: async (cb) => {
        push = cb;
        return () => {};
      },
    });
    await store.start();
    expect(get(store)?.focused?.title).toBe("Proyecto");
    push({ ...state, paused: true });
    expect(get(store)?.paused).toBe(true);
  });
});

describe("stats store", () => {
  it("lee al empezar y cada intervalo; se detiene", async () => {
    vi.useFakeTimers();
    let n = 0;
    const store = createStatsStore({ read: async () => ({ cpu_percent: ++n, ram_used: 1, ram_total: 2 }) }, 1000);
    store.start();
    await vi.advanceTimersByTimeAsync(2100);
    expect(get(store)?.cpu_percent).toBe(3);
    store.stop();
    await vi.advanceTimersByTimeAsync(3000);
    expect(get(store)?.cpu_percent).toBe(3);
  });

  it("una lectura fallida no rompe el store", async () => {
    vi.useFakeTimers();
    const store = createStatsStore({ read: async () => Promise.reject(new Error("x")) }, 1000);
    store.start();
    await vi.advanceTimersByTimeAsync(1500);
    expect(get(store)).toBeNull();
    store.stop();
  });
});
