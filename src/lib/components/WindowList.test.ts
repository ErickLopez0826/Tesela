import { render, screen } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";
import WindowList from "./WindowList.svelte";

describe("WindowList", () => {
  it("muestra ejecutable y título", () => {
    render(WindowList, { windows: [{ handle: 1, exe: "code.exe", title: "Editor" }] });
    expect(screen.getByText("code.exe")).toBeTruthy();
    expect(screen.getByText("Editor")).toBeTruthy();
  });
});
