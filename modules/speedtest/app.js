/* Módulo «Prueba de velocidad». El núcleo ejecuta el CLI de Ookla en segundo plano; aquí solo se inicia, se
 * consulta el estado y se guardan los últimos resultados en `historial.json`. */

var HISTORY_FILE = "historial.json";
var MAX_HISTORY = 10;

function fmt(n) {
  return (Math.round(n * 10) / 10).toFixed(1);
}

function parseHistory(text) {
  try {
    var data = JSON.parse(text);
    return Array.isArray(data) ? data.filter(function (r) { return r && typeof r.download_mbps === "number"; }).slice(0, MAX_HISTORY) : [];
  } catch (e) {
    return [];
  }
}

function addResult(history, result, when) {
  var entry = {
    download_mbps: result.download_mbps,
    upload_mbps: result.upload_mbps,
    ping_ms: result.ping_ms,
    simulated: !!result.simulated,
    when: when,
  };
  return [entry].concat(history).slice(0, MAX_HISTORY);
}

async function start(wm, doc, now) {
  now = now || function () { return new Date().toISOString(); };
  var el = {
    run: doc.getElementById("run"),
    result: doc.getElementById("result"),
    history: doc.getElementById("history"),
    status: doc.getElementById("status"),
  };
  var state = { history: [], timer: null, saved: false };

  function setStatus(text, isError) {
    el.status.textContent = text || "";
    el.status.className = isError ? "error" : "";
  }

  function stat(parent, num, unit) {
    var s = doc.createElement("span");
    var n = doc.createElement("span");
    n.className = "num";
    n.textContent = num;
    var u = doc.createElement("span");
    u.className = "unit";
    u.textContent = " " + unit;
    s.appendChild(n);
    s.appendChild(u);
    parent.appendChild(s);
  }

  function showResult(r) {
    el.result.textContent = "";
    var big = doc.createElement("div");
    big.className = "big";
    stat(big, "↓ " + fmt(r.download_mbps), "Mbps");
    stat(big, "↑ " + fmt(r.upload_mbps), "Mbps");
    stat(big, fmt(r.ping_ms), "ms");
    el.result.appendChild(big);
    var meta = doc.createElement("div");
    meta.className = "meta";
    meta.textContent = [r.server, r.isp].filter(Boolean).join(" · ");
    el.result.appendChild(meta);
    if (r.simulated) {
      var b = doc.createElement("div");
      b.className = "badge";
      b.textContent = "Datos de ejemplo: el CLI de Ookla no está instalado";
      el.result.appendChild(b);
    }
  }

  function showHistory() {
    el.history.textContent = state.history.length > 1
      ? "Anteriores: " + state.history.slice(1, 4).map(function (h) { return fmt(h.download_mbps) + "↓/" + fmt(h.upload_mbps) + "↑"; }).join("  ·  ")
      : "";
  }

  async function poll() {
    var s = await wm.call("speedtest.status");
    if (s.state === "running") {
      el.run.disabled = true;
      setStatus("Midiendo… puede tardar medio minuto.", false);
      return;
    }
    clearInterval(state.timer);
    state.timer = null;
    el.run.disabled = false;
    if (s.state === "done") {
      showResult(s.result);
      setStatus("", false);
      if (!state.saved) {
        state.saved = true;
        state.history = addResult(state.history, s.result, now());
        showHistory();
        await wm.call("files.write", { path: HISTORY_FILE, content: JSON.stringify(state.history) });
      }
    } else if (s.state === "failed") {
      setStatus("La prueba falló: " + s.error, true);
    }
  }

  async function run() {
    setStatus("", false);
    try {
      await wm.call("speedtest.start");
    } catch (e) {
      // Si ya había una en curso simplemente se sigue su estado.
      if (!/en curso|bad_params/.test(String(e.message || e))) { setStatus(String(e.message || e), true); return; }
    }
    state.saved = false;
    el.run.disabled = true;
    if (!state.timer) state.timer = setInterval(function () { poll().catch(function (e) { setStatus(String(e.message || e), true); }); }, 1000);
    await poll();
  }

  el.run.addEventListener("click", function () { run().catch(function (e) { setStatus(String(e.message || e), true); }); });
  wm.onAction(function (id) { if (id === "iniciar") run().catch(function () {}); });

  try {
    var r = await wm.call("files.read", { path: HISTORY_FILE });
    state.history = r ? parseHistory(r.content) : [];
    if (state.history.length > 0) {
      showResult({ download_mbps: state.history[0].download_mbps, upload_mbps: state.history[0].upload_mbps, ping_ms: state.history[0].ping_ms, server: "última medición", isp: state.history[0].when ? state.history[0].when.slice(0, 16).replace("T", " ") : "", simulated: state.history[0].simulated });
      showHistory();
    }
    var s = await wm.call("speedtest.status");
    if (s.state === "running") { await run(); }
  } catch (e) {
    setStatus(String(e.message || e), true);
  }
}

if (typeof window !== "undefined" && window.wm) {
  start(window.wm, document);
}
