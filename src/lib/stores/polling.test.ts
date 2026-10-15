import { get } from "svelte/store";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createPollingStore } from "./polling";

afterEach(() => vi.useRealTimers());

describe("createPollingStore", () => {
  it("lee al empezar, repite y se detiene", async () => {
    vi.useFakeTimers();
    let n = 0;
    const store = createPollingStore(async () => ++n, 1000);
    store.start();
    store.start(); // segunda llamada no duplica el temporizador
    await vi.advanceTimersByTimeAsync(2100);
    expect(get(store)).toBe(3);
    store.stop();
    await vi.advanceTimersByTimeAsync(3000);
    expect(get(store)).toBe(3);
  });

  it("refresh fuerza una lectura", async () => {
    let n = 0;
    const store = createPollingStore(async () => ++n, 60_000);
    await store.refresh();
    await store.refresh();
    expect(get(store)).toBe(2);
  });

  it("ignora lecturas fallidas y conserva el último valor", async () => {
    let fail = false;
    const store = createPollingStore(async () => {
      if (fail) throw new Error("x");
      return "ok";
    }, 60_000);
    await store.refresh();
    fail = true;
    await store.refresh();
    expect(get(store)).toBe("ok");
  });
});
