/* Módulo de ejemplo. Solo usa la API del núcleo (`wm`); no toca Windows. */

function render(doc, state) {
  var out = doc.getElementById("out");
  out.textContent = "";
  function row(label, value, isError) {
    var r = doc.createElement("div");
    r.className = "row";
    var l = doc.createElement("span");
    l.className = "label";
    l.textContent = label;
    var v = doc.createElement("span");
    v.className = isError ? "error" : "";
    v.textContent = value;
    r.appendChild(l);
    r.appendChild(v);
    out.appendChild(r);
  }
  row("Aperturas", state.opens === null ? "—" : String(state.opens), false);
  row("Workspace activo", state.workspace === null ? "—" : String(state.workspace), false);
  row("Ventana enfocada", state.focused || "—", false);
  if (state.error) row("Aviso", state.error, true);
}

/** Arranca el módulo con una API `wm` (la real o una de prueba) y un documento. */
async function start(wm, doc) {
  var state = { opens: null, workspace: null, focused: null, error: null };
  function show() { render(doc, state); }

  // Contador de aperturas, guardado en la carpeta propia del módulo (permiso "files").
  try {
    var prev = await wm.call("files.read", { path: "contador.txt" });
    state.opens = (prev ? parseInt(prev.content, 10) || 0 : 0) + 1;
    await wm.call("files.write", { path: "contador.txt", content: String(state.opens) });
  } catch (e) {
    state.error = String(e.message || e);
  }

  // Estado actual del escritorio (permiso "windows").
  try {
    var ws = await wm.call("windows.active_workspace");
    state.workspace = ws ? ws.number : null;
    var f = await wm.call("windows.focused");
    state.focused = f ? f.title : null;
  } catch (e) {
    state.error = String(e.message || e);
  }
  show();

  // Eventos del núcleo.
  try {
    await wm.on("workspace_changed", function (p) { state.workspace = p ? p.number : null; show(); });
    await wm.on("focus_changed", function (p) { state.focused = p ? p.title : null; show(); });
  } catch (e) {
    state.error = String(e.message || e);
    show();
  }

  // Acción declarada en el manifiesto: el usuario pulsó "Saludar".
  wm.onAction(async function (id) {
    if (id !== "saludar") return;
    try {
      await wm.call("notify.show", { text: "¡Hola desde el módulo! Aperturas: " + state.opens });
    } catch (e) {
      state.error = String(e.message || e);
      show();
    }
  });
}

if (typeof window !== "undefined" && window.wm) {
  start(window.wm, document);
}
