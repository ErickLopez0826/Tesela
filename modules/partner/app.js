/* Módulo «Pareja» (§10.6): mensajes en tiempo real con un avatar animado.
 *
 * Habla con el núcleo por canales (`channels.*`). Hoy el canal es local y simulado: los mensajes no salen de este
 * equipo (hay una «pareja virtual» opcional que responde sola). Cuando exista el servidor de sincronización, el
 * núcleo cambia de backend y este módulo no cambia. */

var PROFILE_FILE = "perfil.json";
var MESSAGES_FILE = "mensajes.json";
var MAX_MESSAGES = 100;
var DEFAULT_ROOM = "demo-pareja";
var BOT_DELAY_MS = 1200;
var MOODS = { idle: "tranquila", happy: "contenta", received: "¡te escribió!" };

function isValidRoom(code) {
  return /^[a-z0-9_-]{1,64}$/.test(code);
}

/** Lo que viaja por el canal: quién (instancia y nombre) y qué texto. */
function encodeMessage(peer, name, text) {
  return JSON.stringify({ peer: peer, name: name, text: text });
}

function decodeMessage(raw) {
  try {
    var m = JSON.parse(raw);
    if (!m || typeof m.peer !== "string" || typeof m.text !== "string") return null;
    return { peer: m.peer, name: typeof m.name === "string" ? m.name.slice(0, 40) : "", text: m.text.slice(0, 2000) };
  } catch (e) {
    return null;
  }
}

function parseProfile(text) {
  var base = { name: "Yo", partnerName: "Mi pareja", room: DEFAULT_ROOM, simulate: true };
  try {
    var d = JSON.parse(text);
    if (d && typeof d === "object") {
      if (typeof d.name === "string" && d.name.trim()) base.name = d.name.trim().slice(0, 40);
      if (typeof d.partnerName === "string" && d.partnerName.trim()) base.partnerName = d.partnerName.trim().slice(0, 40);
      if (typeof d.room === "string" && isValidRoom(d.room)) base.room = d.room;
      if (typeof d.simulate === "boolean") base.simulate = d.simulate;
    }
  } catch (e) { /* perfil por defecto */ }
  return base;
}

function parseMessages(text) {
  try {
    var d = JSON.parse(text);
    return Array.isArray(d)
      ? d.filter(function (m) { return m && (m.from === "me" || m.from === "partner") && typeof m.text === "string"; }).slice(-MAX_MESSAGES)
      : [];
  } catch (e) {
    return [];
  }
}

function addMessage(list, msg) {
  return list.concat([msg]).slice(-MAX_MESSAGES);
}

function newPeerId() {
  return "p" + Math.random().toString(36).slice(2, 10);
}

async function start(wm, doc, options) {
  options = options || {};
  var peer = options.peer || newPeerId();
  var now = options.now || function () { return new Date().toISOString(); };
  var el = {
    avatar: doc.getElementById("avatar"),
    mini: doc.getElementById("face-mini"),
    dot: doc.getElementById("dot"),
    mood: doc.getElementById("mood-text"),
    partnerName: doc.getElementById("partner-name"),
    banner: doc.getElementById("banner"),
    messages: doc.getElementById("messages"),
    form: doc.getElementById("send"),
    text: doc.getElementById("text"),
    quick: doc.getElementById("quick"),
    myName: doc.getElementById("my-name"),
    room: doc.getElementById("room"),
    simulate: doc.getElementById("simulate"),
    save: doc.getElementById("save-settings"),
    status: doc.getElementById("status"),
  };
  var state = { profile: parseProfile("{}"), messages: [], backend: "local", moodTimer: null, subscribedRoom: null };

  function setStatus(text, isError) {
    el.status.textContent = text || "";
    el.status.className = isError ? "error" : "";
  }
  function fail(e) { setStatus(String((e && e.message) || e), true); }

  // ---- avatar ----
  function setMood(mood, ms) {
    ["avatar", "mini"].forEach(function (k) { el[k].setAttribute("class", "face mood-" + mood); });
    el.mood.textContent = MOODS[mood] || "";
    clearTimeout(state.moodTimer);
    if (mood !== "idle") state.moodTimer = setTimeout(function () { setMood("idle"); }, ms || 1500);
  }

  // ---- mensajes ----
  function render() {
    el.messages.textContent = "";
    if (state.messages.length === 0) {
      var e = doc.createElement("div");
      e.className = "empty";
      e.textContent = "Todavía no hay mensajes. Escribe el primero 💌";
      el.messages.appendChild(e);
    }
    state.messages.forEach(function (m) {
      var b = doc.createElement("div");
      b.className = "msg " + m.from;
      b.textContent = m.text;
      var s = doc.createElement("small");
      s.textContent = (m.from === "me" ? state.profile.name : state.profile.partnerName) + (m.at ? " · " + String(m.at).slice(11, 16) : "");
      b.appendChild(s);
      el.messages.appendChild(b);
    });
    el.messages.scrollTop = el.messages.scrollHeight;
    el.partnerName.textContent = state.profile.partnerName;
  }

  async function saveMessages() {
    await wm.call("files.write", { path: MESSAGES_FILE, content: JSON.stringify(state.messages) });
  }
  async function saveProfile() {
    await wm.call("files.write", { path: PROFILE_FILE, content: JSON.stringify(state.profile) });
  }

  async function record(from, text, name) {
    state.messages = addMessage(state.messages, { from: from, text: text, at: now() });
    if (name && from === "partner") state.profile.partnerName = name;
    render();
    await saveMessages();
  }

  async function onChannelMessage(m) {
    var msg = decodeMessage(m.text);
    if (!msg || msg.peer === peer) return; // lo que yo mismo envié vuelve por el canal: se ignora
    await record("partner", msg.text, msg.name);
    setMood("received", 2000);
    el.dot.hidden = false;
    wm.call("notify.show", { text: (msg.name || state.profile.partnerName) + ": " + msg.text.slice(0, 120) }).catch(function () {});
  }

  async function subscribe(room) {
    if (state.subscribedRoom === room) return;
    if (state.subscribedRoom) await wm.call("channels.unsubscribe", { channel: state.subscribedRoom }).catch(function () {});
    await wm.onChannel(room, function (m) { onChannelMessage(m).catch(fail); });
    state.subscribedRoom = room;
  }

  async function send(text) {
    var t = String(text).trim();
    if (!t) return;
    try {
      await wm.call("channels.send", { channel: state.profile.room, text: encodeMessage(peer, state.profile.name, t) });
      await record("me", t);
      setMood("happy", 1200);
      setStatus("", false);
      if (state.backend === "local" && state.profile.simulate) botReply(t);
    } catch (e) {
      fail(e);
    }
  }

  /** Pareja virtual: solo en modo simulado, para ver el avatar y probar sin una segunda persona. */
  function botReply(text) {
    setTimeout(function () {
      wm.call("channels.send", {
        channel: state.profile.room,
        text: encodeMessage("virtual-" + peer, state.profile.partnerName, "(simulación) Recibí: " + text.slice(0, 80)),
      }).catch(function () {});
    }, BOT_DELAY_MS);
  }

  // ---- interfaz ----
  el.form.addEventListener("submit", function (ev) {
    ev.preventDefault();
    var t = el.text.value;
    el.text.value = "";
    send(t);
  });
  ["❤️", "😘", "👋", "🥰"].forEach(function (emoji) {
    var b = doc.createElement("button");
    b.type = "button";
    b.textContent = emoji;
    b.addEventListener("click", function () { send(emoji); });
    el.quick.appendChild(b);
  });
  el.save.addEventListener("click", async function () {
    var room = el.room.value.trim().toLowerCase();
    if (!isValidRoom(room)) { setStatus("El código solo admite minúsculas, números, - y _ (hasta 64).", true); return; }
    state.profile.name = el.myName.value.trim() || "Yo";
    state.profile.room = room;
    state.profile.simulate = el.simulate.checked;
    try {
      await saveProfile();
      await subscribe(room);
      setStatus("Ajustes guardados.", false);
    } catch (e) { fail(e); }
  });
  el.avatar.addEventListener("click", function () { setMood("happy", 1200); el.dot.hidden = true; });
  wm.onAction(function (id) { if (id === "saludar") send("👋"); });

  // ---- arranque ----
  try {
    var p = await wm.call("files.read", { path: PROFILE_FILE });
    state.profile = parseProfile(p ? p.content : "{}");
    var m = await wm.call("files.read", { path: MESSAGES_FILE });
    state.messages = m ? parseMessages(m.content) : [];
    var info = await wm.call("channels.info");
    state.backend = info.backend;
    if (info.backend === "local") {
      el.banner.hidden = false;
      el.banner.textContent = "Modo simulado: los mensajes no salen de este equipo (retardo de " + info.latency_ms + " ms).";
    }
    el.myName.value = state.profile.name;
    el.room.value = state.profile.room;
    el.simulate.checked = state.profile.simulate;
    render();
    await subscribe(state.profile.room);
    setMood("idle");
  } catch (e) {
    render();
    fail(e);
  }
  return { peer: peer, state: state };
}

if (typeof window !== "undefined" && window.wm) {
  start(window.wm, document);
}
