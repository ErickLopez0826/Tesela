import { afterEach, describe, expect, it } from "vitest";
import { clearFavicons, failedCount, faviconSources, isPublicHost, rememberFailed } from "./favicon";

afterEach(clearFavicons);

describe("faviconSources", () => {
  it("primero el favicon de la propia página y luego el servicio de iconos, solo con el servidor", () => {
    expect(faviconSources("https://claude.ai")).toEqual(["https://claude.ai/favicon.ico", "https://www.google.com/s2/favicons?domain=claude.ai&sz=64"]);
    expect(faviconSources("https://gemini.google.com")[1]).toContain("domain=gemini.google.com");
  });

  it("nunca incluye lo que se escribe: solo el origen", () => {
    const sources = faviconSources("https://www.google.com");
    expect(sources.every((s) => !s.includes("search") && !s.includes("?q="))).toBe(true);
  });

  it("los servidores locales no se piden a ningún servicio de fuera", () => {
    expect(faviconSources("http://localhost:3000")).toEqual(["http://localhost:3000/favicon.ico"]);
    expect(faviconSources("http://192.168.1.5")).toEqual(["http://192.168.1.5/favicon.ico"]);
    expect(faviconSources("https://nas.local")).toEqual(["https://nas.local/favicon.ico"]);
  });

  it("solo http y https, y lo que no es una dirección no da nada", () => {
    for (const bad of ["file:///C:/x", "javascript:alert(1)", "data:image/png;base64,AAA", "claude.ai", ""]) {
      expect(faviconSources(bad), bad).toEqual([]);
    }
  });
});

describe("isPublicHost", () => {
  it("distingue internet de la red local", () => {
    expect(isPublicHost("claude.ai")).toBe(true);
    expect(isPublicHost("localhost")).toBe(false);
    expect(isPublicHost("10.0.0.2")).toBe(false);
    expect(isPublicHost("[::1]")).toBe(false);
    expect(isPublicHost("router.home")).toBe(false);
  });
});

describe("memoria de fallos", () => {
  it("recuerda cuántas fuentes fallaron por página", () => {
    expect(failedCount("https://a.com")).toBe(0);
    rememberFailed("https://a.com", 2);
    expect(failedCount("https://a.com")).toBe(2);
    expect(failedCount("https://b.com")).toBe(0);
  });
});
