/* Módulo «Apps abiertas y fijadas» (§9.3). Las abiertas se toman del sistema en tiempo real; las fijadas
 * se guardan en el archivo `fijadas.json` de la carpeta del módulo. */

var PINNED_FILE = "fijadas.json";

/** Normaliza una ruta de Windows para compararla (minúsculas, barras \). */
function normPath(p) {
  return String(p || "").toLowerCase().replace(/\//g, "\\");
}

function baseName(p) {
  var parts = String(p).split(/[\\/]/);
  return parts[parts.length - 1].replace(/\.(exe|lnk)$/i, "");
}

/** Lee la lista de fijadas de un texto JSON; ignora lo que no tenga forma de entrada válida. */
function parsePinned(text) {
  try {
    var data = JSON.parse(text);
    if (!Array.isArray(data)) return [];
    return data
      .filter(function (e) { return e && typeof e.path === "string" && /\.(exe|lnk)$/i.test(e.path); })
      .map(function (e) { return { name: String(e.name || baseName(e.path)), path: e.path }; });
  } catch (e) {
    return [];
  }
}

/** Agrega una fijada sin duplicar (por ruta). Devuelve la lista nueva. */
function pin(list, entry) {
  var key = normPath(entry.path);
  if (list.some(function (e) { return normPath(e.path) === key; })) return list;
  return list.concat([{ name: entry.name || baseName(entry.path), path: entry.path }]);
}

function unpin(list, path) {
  var key = normPath(path);
  return list.filter(function (e) { return normPath(e.path) !== key; });
}

/** ¿Hay alguna ventana abierta de esa app fijada? */
function openWindowFor(openApps, pinned) {
  var key = normPath(pinned.path);
  return openApps.filter(function (a) { return normPath(a.path) === key; })[0] || null;
}

async function start(wm, doc) {
  var el = {
    items: doc.getElementById("items"),
    tabOpen: doc.getElementById("tab-open"),
    tabPinned: doc.getElementById("tab-pinned"),
    add: doc.getElementById("add"),
    path: doc.getElementById("path"),
    status: doc.getElementById("status"),
  };
  var state = { tab: "open", open: [], pinned: [] };

  function setStatus(text, isError) {
    el.status.textContent = text || "";
    el.status.className = isError ? "error" : "";
  }
  function fail(e) { setStatus(String((e && e.message) || e), true); }

  async function loadPinned() {
    var r = await wm.call("files.read", { path: PINNED_FILE });
    state.pinned = r ? parsePinned(r.content) : [];
  }
  async function savePinned() {
    await wm.call("files.write", { path: PINNED_FILE, content: JSON.stringify(state.pinned) });
  }
  async function loadOpen() {
    state.open = await wm.call("apps.list_open");
  }

  function button(label, onClick, cls) {
    var b = doc.createElement("button");
    b.type = "button";
    b.textContent = label;
    if (cls) b.className = cls;
    b.addEventListener("click", function () { Promise.resolve(onClick()).catch(fail); });
    return b;
  }

  function row(nameText, subText, buttons) {
    var r = doc.createElement("div");
    r.className = "item";
    var n = doc.createElement("div");
    n.className = "name";
    n.textContent = nameText;
    if (subText) {
      var s = doc.createElement("small");
      s.textContent = subText;
      n.appendChild(s);
    }
    r.appendChild(n);
    buttons.forEach(function (b) { r.appendChild(b); });
    return r;
  }

  function render() {
    el.tabOpen.className = state.tab === "open" ? "active" : "";
    el.tabPinned.className = state.tab === "pinned" ? "active" : "";
    el.add.hidden = state.tab !== "pinned";
    el.items.textContent = "";
    if (state.tab === "open") {
      if (state.open.length === 0) {
        var e = doc.createElement("div");
        e.className = "empty";
        e.textContent = "No hay apps abiertas.";
        el.items.appendChild(e);
      }
      state.open.forEach(function (a) {
        var buttons = [button("Ir", function () { return wm.call("apps.focus", { handle: a.handle }); })];
        var already = a.path && state.pinned.some(function (p) { return normPath(p.path) === normPath(a.path); });
        if (a.path && !already) {
          buttons.push(button("Fijar", async function () {
            state.pinned = pin(state.pinned, { name: baseName(a.path), path: a.path });
            await savePinned();
            setStatus("Fijada: " + baseName(a.path), false);
            render();
          }));
        }
        el.items.appendChild(row(a.title, a.exe, buttons));
      });
    } else {
      if (state.pinned.length === 0) {
        var e2 = doc.createElement("div");
        e2.className = "empty";
        e2.textContent = "Fija apps desde «Abiertas» o escribe su ruta abajo.";
        el.items.appendChild(e2);
      }
      state.pinned.forEach(function (p) {
        var running = openWindowFor(state.open, p);
        var go = running
          ? button("Ir", function () { return wm.call("apps.focus", { handle: running.handle }); })
          : button("Abrir", function () { return wm.call("apps.launch", { path: p.path }); });
        var rm = button("Quitar", async function () {
          state.pinned = unpin(state.pinned, p.path);
          await savePinned();
          render();
        });
        el.items.appendChild(row(p.name, running ? "abierta" : p.path, [go, rm]));
      });
    }
  }

  function setTab(t) {
    state.tab = t;
    render();
  }

  el.tabOpen.addEventListener("click", function () { setTab("open"); });
  el.tabPinned.addEventListener("click", function () { setTab("pinned"); });
  wm.onAction(function (id) { if (id === "alternar") setTab(state.tab === "open" ? "pinned" : "open"); });
  el.add.addEventListener("submit", function (ev) {
    ev.preventDefault();
    var path = el.path.value.trim();
    if (!/^[a-zA-Z]:[\\/].*\.(exe|lnk)$/i.test(path)) {
      setStatus("Escribe la ruta completa de un .exe o .lnk", true);
      return;
    }
    state.pinned = pin(state.pinned, { name: baseName(path), path: path });
    el.path.value = "";
    savePinned().then(function () { setStatus("Fijada: " + baseName(path), false); render(); }).catch(fail);
  });

  try { await loadPinned(); } catch (e) { fail(e); }
  try { await loadOpen(); } catch (e) { fail(e); }
  render();
  // Las abiertas se refrescan en tiempo real.
  setInterval(function () {
    loadOpen().then(render).catch(function () {});
  }, 2000);
}

if (typeof window !== "undefined" && window.wm) {
  start(window.wm, document);
}
