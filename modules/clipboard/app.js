/* Módulo «Historial del portapapeles». Consulta el portapapeles cada segundo y guarda un historial corto en
 * `historial.json`. El núcleo no entrega los textos que los gestores de contraseñas marcan como privados. */

var HISTORY_FILE = "historial.json";
var MAX_ENTRIES = 50;
var MAX_ENTRY_CHARS = 5000;

/** Agrega un texto al principio del historial: sin duplicados (sube si ya estaba), sin vacíos ni textos enormes. */
function addEntry(history, text) {
  var t = String(text);
  if (t.trim() === "" || t.length > MAX_ENTRY_CHARS) return history;
  var rest = history.filter(function (h) { return h !== t; });
  return [t].concat(rest).slice(0, MAX_ENTRIES);
}

function parseHistory(text) {
  try {
    var data = JSON.parse(text);
    return Array.isArray(data) ? data.filter(function (x) { return typeof x === "string"; }).slice(0, MAX_ENTRIES) : [];
  } catch (e) {
    return [];
  }
}

function preview(text) {
  var one = String(text).replace(/\s+/g, " ").trim();
  return one.length > 90 ? one.slice(0, 87) + "…" : one;
}

async function start(wm, doc) {
  var el = {
    items: doc.getElementById("items"),
    count: doc.getElementById("count"),
    status: doc.getElementById("status"),
  };
  var state = { history: [], lastSeen: null, copiedText: null };

  async function load() {
    var r = await wm.call("files.read", { path: HISTORY_FILE });
    state.history = r ? parseHistory(r.content) : [];
  }

  function render() {
    el.count.textContent = String(state.history.length);
    el.items.textContent = "";
    if (state.history.length === 0) {
      var e = doc.createElement("div");
      e.className = "empty";
      e.textContent = "Todavía no has copiado nada.";
      el.items.appendChild(e);
      return;
    }
    state.history.forEach(function (text) {
      var b = doc.createElement("button");
      b.type = "button";
      b.className = "entry" + (text === state.copiedText ? " copied" : "");
      b.textContent = preview(text);
      b.title = text.slice(0, 400);
      b.addEventListener("click", function () {
        state.copiedText = text;
        state.lastSeen = text; // no se vuelve a registrar lo que acabamos de copiar nosotros
        wm.call("clipboard.write", { text: text })
          .then(function () { el.status.textContent = "Copiado."; el.status.className = ""; render(); })
          .catch(function (err) { el.status.textContent = String(err.message || err); el.status.className = "error"; });
      });
      el.items.appendChild(b);
    });
  }

  /** Una pasada: lee el portapapeles y, si hay algo nuevo, lo suma al historial compartido en disco. */
  async function tick() {
    var r = await wm.call("clipboard.read");
    var text = r && typeof r.text === "string" ? r.text : null;
    if (text === null || text === state.lastSeen) return;
    state.lastSeen = text;
    // La barra y el panel ejecutan este mismo módulo: se relee el archivo antes de escribir para no pisarse.
    await load();
    var next = addEntry(state.history, text);
    if (next !== state.history) {
      state.history = next;
      await wm.call("files.write", { path: HISTORY_FILE, content: JSON.stringify(state.history) });
    }
    render();
  }

  wm.onAction(async function (id) {
    if (id !== "limpiar") return;
    state.history = [];
    state.copiedText = null;
    await wm.call("files.write", { path: HISTORY_FILE, content: "[]" });
    render();
  });

  try { await load(); } catch (e) { el.status.textContent = String(e.message || e); el.status.className = "error"; }
  render();
  await tick().catch(function () {});
  setInterval(function () { tick().catch(function () {}); }, 1000);
}

if (typeof window !== "undefined" && window.wm) {
  start(window.wm, document);
}
