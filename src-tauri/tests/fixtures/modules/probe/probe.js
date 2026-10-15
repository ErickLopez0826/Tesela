/* Llama a todo lo que el núcleo ofrece; el anfitrión registra qué se permitió y qué no. */
(async function () {
  var calls = [
    ["core.version", null],
    ["windows.active_workspace", null],
    ["windows.focused", null],
    ["system.stats", null],
    ["power.run", { action: "shutdown" }],
    ["files.write", { path: "sonda.txt", content: "dentro" }],
    ["files.write", { path: "../fuera.txt", content: "escape" }],
    ["files.read", { path: "sonda.txt" }],
    ["notify.show", { text: "hola desde la sonda" }],
    ["events.subscribe", { event: "workspace_changed" }],
    ["events.subscribe", { event: "evento-inventado" }],
    ["metodo.inventado", null],
  ];
  for (var i = 0; i < calls.length; i++) {
    try { await wm.call(calls[i][0], calls[i][1]); } catch (e) { /* el anfitrión lo registra */ }
  }
  // Intento de saltarse el SDK: hablar con Tauri directamente desde el iframe aislado. Se usa un comando de
  // solo lectura; si el núcleo lo dejara pasar, el módulo habría escapado de su aislamiento.
  var direct = { available: typeof window.__TAURI_INTERNALS__ !== "undefined", result: "no disponible" };
  if (direct.available) {
    try {
      // Si Tauri no contesta jamás, el comando queda bloqueado igualmente: no se obtiene ningún dato.
      var answer = await Promise.race([
        window.__TAURI_INTERNALS__.invoke("modules_list"),
        new Promise(function (resolve) { setTimeout(function () { resolve("__sin_respuesta__"); }, 8000); }),
      ]);
      direct.result = answer === "__sin_respuesta__" ? "BLOQUEADO: sin respuesta" : "PERMITIDO";
    } catch (e) {
      direct.result = "BLOQUEADO: " + String(e);
    }
  }
  try { await wm.call("probe.tauri_directo", direct); } catch (e) {}
  try { await wm.call("probe.done", null); } catch (e) {}
})();
