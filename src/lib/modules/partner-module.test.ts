import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import code from "../../../modules/partner/app.js?raw";
import html from "../../../modules/partner/index.html?raw";
import manifest from "../../../modules/partner/module.json";

// El módulo corre aquí contra una API `wm` falsa y un "canal" en memoria con retardo. Dos instancias, cada una con su
// propio documento, se envían mensajes como lo harían dos equipos.
type Msg = { channel: string; from: string; text: string; sent_at_ms: number };
type Api = {
  start: (wm: unknown, doc: Document, opts?: { peer?: string; now?: () => string }) => Promise<{ peer: string; state: any }>;
  isValidRoom: (c: string) => boolean;
  encodeMessage: (p: string, n: string, t: string) => string;
  decodeMessage: (r: string) => { peer: string; name: string; text: string } | null;
  parseProfile: (t: string) => { name: string; partnerName: string; room: string; simulate: boolean };
  parseMessages: (t: string) => { from: string; text: string }[];
  addMessage: (l: unknown[], m: unknown) => unknown[];
};
const api = new Function(`${code}\nreturn { start, isValidRoom, encodeMessage, decodeMessage, parseProfile, parseMessages, addMessage };`)() as Api;

function makeHub(latency: number) {
  const subs: { channel: string; cb: (m: Msg) => void; owner: string }[] = [];
  return {
    subs,
    subscribe: (owner: string, channel: string, cb: (m: Msg) => void) => void subs.push({ owner, channel, cb }),
    unsubscribe: (owner: string, channel: string) => {
      for (let i = subs.length - 1; i >= 0; i--) if (subs[i].owner === owner && subs[i].channel === channel) subs.splice(i, 1);
    },
    send(channel: string, text: string, from: string) {
      setTimeout(() => subs.filter((s) => s.channel === channel).forEach((s) => s.cb({ channel, from, text, sent_at_ms: 0 })), latency);
    },
  };
}

function makeDoc(): Document {
  return new DOMParser().parseFromString(html, "text/html");
}

function makeInstance(hub: ReturnType<typeof makeHub>, owner: string, backend: "local" | "remote", files: Record<string, string> = {}) {
  const fs = new Map(Object.entries(files));
  const sent: { channel: string; text: string }[] = [];
  const notices: string[] = [];
  let action: (id: string) => void = () => {};
  const wm = {
    call: vi.fn(async (method: string, p?: any) => {
      switch (method) {
        case "files.read": return fs.has(p.path) ? { content: fs.get(p.path) } : null;
        case "files.write": return (fs.set(p.path, p.content), true);
        case "channels.info": return { backend, latency_ms: 400 };
        case "channels.send": sent.push(p); hub.send(p.channel, p.text, owner); return true;
        case "channels.unsubscribe": return (hub.unsubscribe(owner, p.channel), true);
        case "notify.show": return (notices.push(p.text), null);
        default: throw new Error(`unknown_method: ${method}`);
      }
    }),
    onChannel: vi.fn(async (channel: string, h: (m: Msg) => void) => (hub.subscribe(owner, channel, h), true)),
    onAction: vi.fn((h: (id: string) => void) => void (action = h)),
  };
  const doc = makeDoc();
  return { wm, doc, fs, sent, notices, act: (id: string) => action(id), q: (s: string) => doc.querySelector(s) as HTMLElement, texts: () => [...doc.querySelectorAll(".msg")].map((m) => m.firstChild?.textContent) };
}

const tick = (ms: number) => vi.advanceTimersByTimeAsync(ms);

async function boot(i: ReturnType<typeof makeInstance>, peer: string) {
  const r = await api.start(i.wm, i.doc, { peer, now: () => "2026-10-15T09:30:00" });
  return r;
}

function send(i: ReturnType<typeof makeInstance>, text: string) {
  (i.q("#text") as HTMLInputElement).value = text;
  i.q("#send").dispatchEvent(new Event("submit", { cancelable: true }));
}

beforeEach(() => vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval"] }));
afterEach(() => vi.useRealTimers());

describe("módulo Pareja: lógica", () => {
  it("el manifiesto declara canales, archivos y avisos, y un hueco en la barra", () => {
    expect([...manifest.permissions].sort()).toEqual(["channels", "files", "notifications"]);
    expect(manifest.ui).toHaveProperty("bar");
    expect(manifest.actions[0].id).toBe("saludar");
  });

  it("códigos de pareja válidos", () => {
    expect(api.isValidRoom("nuestra-sala_1")).toBe(true);
    for (const bad of ["", "Mayus", "con espacio", "a/b", "x".repeat(65)]) expect(api.isValidRoom(bad)).toBe(false);
  });

  it("codifica y decodifica mensajes; descarta lo mal formado", () => {
    const raw = api.encodeMessage("p1", "Erick", "hola");
    expect(api.decodeMessage(raw)).toEqual({ peer: "p1", name: "Erick", text: "hola" });
    expect(api.decodeMessage("no json")).toBeNull();
    expect(api.decodeMessage('{"peer":1,"text":"x"}')).toBeNull();
    expect(api.decodeMessage('{"peer":"p","text":"' + "x".repeat(3000) + '"}')!.text.length).toBe(2000);
  });

  it("perfil e historial con valores por defecto y validación", () => {
    expect(api.parseProfile("{}")).toEqual({ name: "Yo", partnerName: "Mi pareja", room: "demo-pareja", simulate: true });
    expect(api.parseProfile('{"name":" Erick ","room":"MAL CÓDIGO","simulate":false}')).toMatchObject({ name: "Erick", room: "demo-pareja", simulate: false });
    expect(api.parseProfile("roto").room).toBe("demo-pareja");
    expect(api.parseMessages('[{"from":"me","text":"a"},{"from":"otro","text":"b"},5]')).toHaveLength(1);
    let list: unknown[] = [];
    for (let i = 0; i < 130; i++) list = api.addMessage(list, { from: "me", text: String(i) });
    expect(list).toHaveLength(100);
  });
});

describe("módulo Pareja: dos instancias", () => {
  it("se envían mensajes con retardo, el avatar reacciona y no se duplica lo propio", async () => {
    const hub = makeHub(300);
    const a = makeInstance(hub, "A", "remote");
    const b = makeInstance(hub, "B", "remote");
    await boot(a, "peer-a");
    await boot(b, "peer-b");
    expect(a.q("#banner").hidden).toBe(true);

    send(a, "hola mi amor");
    await tick(0);
    expect(a.texts()).toEqual(["hola mi amor"]);
    expect(a.q("#avatar").getAttribute("class")).toContain("mood-happy");
    expect(b.texts()).toEqual([]); // todavía en camino: hay retardo
    await tick(299);
    expect(b.texts()).toEqual([]);
    await tick(10);
    expect(b.texts()).toEqual(["hola mi amor"]);
    expect(b.q(".msg").className).toContain("partner");
    expect(b.q("#avatar").getAttribute("class")).toContain("mood-received");
    expect(b.q("#mood-text").textContent).toBe("¡te escribió!");
    expect(b.q("#dot").hidden).toBe(false);
    expect(b.notices[0]).toContain("hola mi amor");
    // El eco de lo que A envió no se muestra otra vez en A.
    expect(a.texts()).toEqual(["hola mi amor"]);
    expect(a.notices).toEqual([]);

    // Y en sentido contrario.
    send(b, "te quiero");
    await tick(400);
    expect(a.texts()).toEqual(["hola mi amor", "te quiero"]);
    expect(b.texts()).toEqual(["hola mi amor", "te quiero"]);
    // Todo queda guardado en su carpeta.
    expect(JSON.parse(a.fs.get("mensajes.json")!).map((m: any) => m.from)).toEqual(["me", "partner"]);
  });

  it("el avatar vuelve a su estado tranquilo", async () => {
    const hub = makeHub(10);
    const a = makeInstance(hub, "A", "remote");
    await boot(a, "peer-a");
    send(a, "x");
    await tick(0);
    expect(a.q("#avatar").getAttribute("class")).toContain("mood-happy");
    await tick(1300);
    expect(a.q("#avatar").getAttribute("class")).toBe("face mood-idle");
    expect(a.q("#face-mini").getAttribute("class")).toBe("face mood-idle");
  });

  it("la acción «saludar» y los botones rápidos envían", async () => {
    const hub = makeHub(10);
    const a = makeInstance(hub, "A", "remote");
    await boot(a, "peer-a");
    a.act("saludar");
    await tick(0);
    (a.doc.querySelectorAll("#quick button")[0] as HTMLButtonElement).click();
    await tick(0);
    expect(a.sent.map((s) => api.decodeMessage(s.text)!.text)).toEqual(["👋", "❤️"]);
  });

  it("no envía mensajes vacíos", async () => {
    const hub = makeHub(10);
    const a = makeInstance(hub, "A", "remote");
    await boot(a, "peer-a");
    send(a, "   ");
    await tick(0);
    expect(a.sent).toEqual([]);
  });
});

describe("módulo Pareja: modo simulado", () => {
  it("avisa de que los mensajes no salen del equipo y la pareja virtual responde", async () => {
    const hub = makeHub(400);
    const a = makeInstance(hub, "A", "local");
    await boot(a, "peer-a");
    expect(a.q("#banner").hidden).toBe(false);
    expect(a.q("#banner").textContent).toContain("Modo simulado");
    send(a, "buenos días");
    await tick(0);
    expect(a.texts()).toEqual(["buenos días"]);
    await tick(1200 + 400 + 50); // la pareja virtual tarda y además hay retardo de canal
    expect(a.texts()).toEqual(["buenos días", "(simulación) Recibí: buenos días"]);
    expect(a.q("#avatar").getAttribute("class")).toContain("mood-received");
  });

  it("la pareja virtual no aparece con un backend real ni si se desactiva", async () => {
    const hub = makeHub(10);
    const remote = makeInstance(hub, "R", "remote");
    await boot(remote, "p-r");
    send(remote, "hola");
    await tick(3000);
    expect(remote.texts()).toEqual(["hola"]);
    const hub2 = makeHub(10);
    const off = makeInstance(hub2, "O", "local", { "perfil.json": JSON.stringify({ simulate: false }) });
    await boot(off, "p-o");
    send(off, "hola");
    await tick(3000);
    expect(off.texts()).toEqual(["hola"]);
  });
});

describe("módulo Pareja: ajustes y persistencia", () => {
  it("restaura el historial y el perfil guardados", async () => {
    const hub = makeHub(10);
    const a = makeInstance(hub, "A", "remote", {
      "perfil.json": JSON.stringify({ name: "Erick", partnerName: "Ana", room: "nuestra-sala" }),
      "mensajes.json": JSON.stringify([{ from: "partner", text: "buenas", at: "2026-10-14T20:15:00" }]),
    });
    await boot(a, "peer-a");
    expect(a.texts()).toEqual(["buenas"]);
    expect(a.q("#partner-name").textContent).toBe("Ana");
    expect((a.q("#room") as HTMLInputElement).value).toBe("nuestra-sala");
    expect(hub.subs.map((s) => s.channel)).toEqual(["nuestra-sala"]);
  });

  it("cambiar el código de pareja valida, guarda y cambia de canal", async () => {
    const hub = makeHub(10);
    const a = makeInstance(hub, "A", "remote");
    await boot(a, "peer-a");
    (a.q("#room") as HTMLInputElement).value = "Código Malo";
    a.q("#save-settings").click();
    await tick(0);
    expect(a.q("#status").className).toBe("error");
    expect(hub.subs.map((s) => s.channel)).toEqual(["demo-pareja"]);
    (a.q("#room") as HTMLInputElement).value = "Sala-Nueva";
    (a.q("#my-name") as HTMLInputElement).value = "Erick";
    a.q("#save-settings").click();
    await tick(0);
    expect(hub.subs.map((s) => s.channel)).toEqual(["sala-nueva"]);
    expect(JSON.parse(a.fs.get("perfil.json")!)).toMatchObject({ name: "Erick", room: "sala-nueva" });
    send(a, "ya en la sala nueva");
    await tick(0);
    expect(a.sent[0].channel).toBe("sala-nueva");
  });

  it("si el núcleo niega el permiso, lo dice y no se rompe", async () => {
    const hub = makeHub(10);
    const a = makeInstance(hub, "A", "remote");
    a.wm.call.mockImplementation(async (m: string) => {
      throw new Error(`permission_denied: '${m}' necesita el permiso 'files', que no se concedió`);
    });
    await boot(a, "peer-a");
    expect(a.q("#status").textContent).toContain("permission_denied");
    expect(a.q("#messages").textContent).toContain("Todavía no hay mensajes");
  });
});
