// Hook de Claude Code para Tesela: anota qué ocurre en cada sesión (trabajando, esperando, parada) en un archivo por sesión.
// Tesela lo lee para enseñar el estado en la barra. No hace red, no lee credenciales y no imprime nada: solo escribe un archivo
// pequeño en %LOCALAPPDATA%\Tesela\claude\sessions (o en TESELA_CLAUDE_DIR, para las pruebas). Además del estado anota el nombre
// de la carpeta del proyecto, desde dónde se usa Claude Code (terminal, VS Code…) y los subagentes que están en marcha; nunca el
// texto de lo que se pide ni de lo que se responde.
// Reglas duras: todo dentro de try/catch, nunca escribe en stdout/stderr y siempre sale con código 0 (un fallo aquí no debe
// molestar nunca a Claude Code).
"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");

function dataDir() {
  if (process.env.TESELA_CLAUDE_DIR) return process.env.TESELA_CLAUDE_DIR;
  const base = process.env.LOCALAPPDATA || path.join(os.homedir(), "AppData", "Local");
  return path.join(base, "Tesela", "claude");
}

/** El identificador de la sesión se usa como nombre de archivo: solo letras, números, `_` y `-`. */
function safeId(id) {
  return typeof id === "string" && /^[A-Za-z0-9_-]{1,100}$/.test(id) ? id : null;
}

/** Un subagente sin avisos durante este tiempo se da por terminado (por si no llegó su final). */
const AGENT_STALE_SECONDS = 300;
/** Con estos avisos empieza un turno nuevo o termina el actual: no queda ningún subagente en marcha. */
const CLEARS_AGENTS = new Set(["UserPromptSubmit", "Stop", "StopFailure", "SessionStart"]);

/** Un texto corto y sin saltos de línea, o `null`. */
function shortText(value, max) {
  if (typeof value !== "string") return null;
  const text = value.replace(/[\u0000-\u001f]/g, " ").trim();
  return text.length > 0 && text.length <= max ? text : null;
}

/** El nombre de la carpeta de trabajo (no la ruta entera). */
function projectName(cwd) {
  if (typeof cwd !== "string") return null;
  const parts = cwd.split(/[\\/]/).filter(Boolean);
  return shortText(parts[parts.length - 1] || "", 60);
}

/** Subagentes que siguen en marcha tras este aviso. */
function nextAgents(previous, payload, event, nowSeconds) {
  if (CLEARS_AGENTS.has(event)) return [];
  const kept = (Array.isArray(previous) ? previous : []).filter(
    (a) => a && typeof a.id === "string" && typeof a.type === "string" && typeof a.at === "number" && nowSeconds - a.at <= AGENT_STALE_SECONDS,
  );
  const id = shortText(payload.agent_id, 100);
  if (!id) return kept;
  const rest = kept.filter((a) => a.id !== id);
  if (event === "SubagentStop") return rest;
  const type = shortText(payload.agent_type, 60);
  return type ? [...rest, { id, type, at: nowSeconds }] : kept;
}

/** Lo que se anota de un aviso, o `null` si no sirve. `previous` es lo anotado antes en esa sesión; `env`, el entorno de Claude Code. */
function buildRecord(payload, nowSeconds, previous = null, env = process.env) {
  if (!payload || typeof payload !== "object") return null;
  const event = payload.hook_event_name;
  if (typeof event !== "string" || event.length === 0 || event.length > 60) return null;
  const record = { event, at: nowSeconds };
  if (typeof payload.notification_type === "string" && payload.notification_type.length <= 60) {
    record.notification_type = payload.notification_type;
  }
  const project = projectName(payload.cwd);
  if (project) record.project = project;
  const origin = typeof env.CLAUDE_CODE_ENTRYPOINT === "string" && /^[A-Za-z0-9_-]{1,40}$/.test(env.CLAUDE_CODE_ENTRYPOINT) ? env.CLAUDE_CODE_ENTRYPOINT : null;
  if (origin) record.origin = origin;
  const agents = nextAgents(previous && previous.agents, payload, event, nowSeconds);
  if (agents.length > 0) record.agents = agents;
  return record;
}

/** Aplica un aviso: escribe el archivo de la sesión (de forma atómica) o lo borra si la sesión terminó. */
function apply(payload, dir, nowSeconds, env = process.env) {
  const id = safeId(payload && payload.session_id);
  if (!id) return false;
  const sessions = path.join(dir, "sessions");
  const file = path.join(sessions, id + ".json");
  let previous = null;
  try {
    previous = JSON.parse(fs.readFileSync(file, "utf8"));
  } catch {
    // primera vez, o a medias: se empieza de cero
  }
  const record = buildRecord(payload, nowSeconds, previous, env);
  if (!record) return false;
  if (record.event === "SessionEnd") {
    try {
      fs.unlinkSync(file);
    } catch {
      // ya no estaba
    }
    return true;
  }
  fs.mkdirSync(sessions, { recursive: true });
  const tmp = file + "." + process.pid + ".tmp";
  fs.writeFileSync(tmp, JSON.stringify(record));
  fs.renameSync(tmp, file);
  return true;
}

function main() {
  const quit = () => process.exit(0);
  // Si stdin no llega, no se queda colgado.
  const guard = setTimeout(quit, 3000);
  guard.unref();
  let text = "";
  process.stdin.setEncoding("utf8");
  process.stdin.on("data", (c) => (text += c));
  process.stdin.on("error", quit);
  process.stdin.on("end", () => {
    try {
      apply(JSON.parse(text.replace(/^﻿/, "")), dataDir(), Math.floor(Date.now() / 1000));
    } catch {
      // un fallo aquí nunca molesta a Claude Code
    }
    quit();
  });
}

if (require.main === module) {
  try {
    main();
  } catch {
    process.exit(0);
  }
}

module.exports = { safeId, buildRecord, apply };
