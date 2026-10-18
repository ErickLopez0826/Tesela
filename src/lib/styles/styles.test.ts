import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const root = join(process.cwd(), "src");

function files(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) files(path, out);
    else if (/\.(svelte|css)$/.test(name)) out.push(path);
  }
  return out;
}

const sources = files(root).map((path) => ({ path, text: readFileSync(path, "utf8") }));

describe("estilos globales", () => {
  it("ninguna @property registrada pisa una variable del tema", () => {
    // Una @property vale para todo el documento (todas las ventanas comparten la misma hoja): si registra `--fg` como número,
    // el `--fg: #f1f1f1` del tema deja de valer y el texto se vuelve negro.
    const registered = sources.flatMap(({ text }) => [...text.matchAll(/@property\s+(--[\w-]+)/g)].map((m) => m[1]));
    const declared = new Set(sources.flatMap(({ path, text }) => (/\.css$/.test(path) ? [...text.matchAll(/(--[\w-]+)\s*:/g)].map((m) => m[1]) : [])));
    const theme = ["--fg", "--muted", "--bg", "--line", "--panel", "--accent", "--on-accent", "--mono", "--frame", "--dot", "--ws", "--ws-on"];
    for (const name of registered) {
      expect(declared.has(name), `${name} choca con una variable de los estilos globales`).toBe(false);
      expect(theme.includes(name), `${name} choca con una variable del tema`).toBe(false);
    }
    expect(registered.length).toBeGreaterThan(0);
  });

  it("el resalte de un workspace tiene la misma forma que la pastilla del actual", () => {
    const strip = sources.find((s) => s.path.endsWith("WorkspaceStrip.svelte"))!.text;
    const ws = /^\s*\.ws \{ border-radius: ([^;]+);/m.exec(strip)?.[1];
    const pill = /\.pill \{[^}]*border-radius: ([^;]+);/.exec(strip)?.[1];
    expect(ws).toBe("999px");
    expect(pill).toBe(ws);
  });
});
