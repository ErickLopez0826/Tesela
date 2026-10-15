import { get } from "svelte/store";
import { describe, expect, it } from "vitest";
import { createWindowsStore } from "./windows";

describe("windows store", () => {
  it("carga la lista de ventanas", async () => {
    const store = createWindowsStore(async () => [{ handle: 1, exe: "code.exe", title: "Editor" }]);
    await store.refresh();
    expect(get(store).windows).toEqual([{ handle: 1, exe: "code.exe", title: "Editor" }]);
    expect(get(store).loading).toBe(false);
  });

  it("guarda el error si falla", async () => {
    const store = createWindowsStore(async () => {
      throw new Error("fallo");
    });
    await store.refresh();
    expect(get(store).error).toContain("fallo");
    expect(get(store).windows).toEqual([]);
  });
});
