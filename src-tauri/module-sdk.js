/* SDK de módulos de Tesela. Lo sirve el núcleo en /__wm/sdk.js: los módulos no lo copian.
 *
 * El módulo corre en un iframe aislado (sandbox, sin acceso a Windows ni a Tauri). Todo lo que quiera
 * del sistema lo pide al núcleo con `wm.call(método, parámetros)`; el núcleo comprueba sus permisos.
 *
 *   wm.call("windows.active_workspace")          -> { number: 1 }
 *   wm.on("workspace_changed", (p) => ...)       -> recibe eventos (requiere haberlos pedido con subscribe)
 *   wm.onAction((id) => ...)                     -> el usuario pulsó una acción que el módulo declaró
 *   wm.onChannel("pareja", (m) => ...)           -> mensajes de un canal (permiso "channels"); enviar con wm.call("channels.send", ...)
 *   wm.ready                                     -> promesa que se resuelve cuando el núcleo responde
 */
(function () {
  "use strict";
  var pending = new Map();
  var handlers = new Map();
  var actionHandlers = [];
  var nextId = 1;
  var TIMEOUT_MS = 15000;

  window.addEventListener("message", function (ev) {
    var m = ev.data;
    if (ev.source !== window.parent || !m || m.wm !== 1) return;
    if (typeof m.id === "number" && pending.has(m.id)) {
      var p = pending.get(m.id);
      pending.delete(m.id);
      clearTimeout(p.timer);
      if (m.error !== undefined) p.reject(new Error(String(m.error)));
      else p.resolve(m.result);
    } else if (typeof m.event === "string") {
      (handlers.get(m.event) || []).forEach(function (h) {
        try { h(m.payload); } catch (e) { console.error(e); }
      });
    } else if (typeof m.action === "string") {
      actionHandlers.forEach(function (h) {
        try { h(m.action); } catch (e) { console.error(e); }
      });
    }
  });

  function call(method, params) {
    return new Promise(function (resolve, reject) {
      var id = nextId++;
      var timer = setTimeout(function () {
        pending.delete(id);
        reject(new Error("el núcleo no respondió a " + method));
      }, TIMEOUT_MS);
      pending.set(id, { resolve: resolve, reject: reject, timer: timer });
      window.parent.postMessage({ wm: 1, id: id, method: method, params: params === undefined ? null : params }, "*");
    });
  }

  window.wm = {
    call: call,
    /** Pide recibir un evento del núcleo y registra el manejador. */
    on: function (event, handler) {
      var list = handlers.get(event) || [];
      list.push(handler);
      handlers.set(event, list);
      return call("events.subscribe", { event: event });
    },
    off: function (event) {
      handlers.delete(event);
      return call("events.unsubscribe", { event: event });
    },
    onAction: function (handler) {
      actionHandlers.push(handler);
    },
    /** Se suscribe a un canal del núcleo (permiso "channels"); el manejador recibe { channel, from, text, sent_at_ms }. */
    onChannel: function (channel, handler) {
      var list = handlers.get("channel_message") || [];
      list.push(function (p) { if (p && p.channel === channel) handler(p); });
      handlers.set("channel_message", list);
      return call("channels.subscribe", { channel: channel });
    },
    ready: call("core.version"),
  };
})();
