// Pruebas del hook con el ejecutor integrado de Node: `node --test scripts/`.
"use strict";
const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawnSync } = require("child_process");
const { safeId, buildRecord, apply } = require("./claude-hook.cjs");

// El entorno de quien ejecuta las pruebas puede ser el propio Claude Code: sin su origen, para que no se cuele en lo anotado.
delete process.env.CLAUDE_CODE_ENTRYPOINT;
const tmp = () => fs.mkdtempSync(path.join(os.tmpdir(), "tesela-hook-"));
const SCRIPT = path.join(__dirname, "claude-hook.cjs");

test("solo se aceptan identificadores de sesión seguros", () => {
  assert.strictEqual(safeId("abc-123_X"), "abc-123_X");
  for (const bad of ["", "../x", "a/b", "a\b", "a b", "a.json", null, undefined, 5, "x".repeat(101)]) assert.strictEqual(safeId(bad), null, String(bad));
});

test("se anota el evento y, si lo hay, el tipo de aviso", () => {
  assert.deepStrictEqual(buildRecord({ hook_event_name: "Stop" }, 10), { event: "Stop", at: 10 });
  assert.deepStrictEqual(buildRecord({ hook_event_name: "Notification", notification_type: "permission_prompt", message: "secreto" }, 11), {
    event: "Notification",
    at: 11,
    notification_type: "permission_prompt",
  });
  for (const bad of [null, "x", {}, { hook_event_name: "" }, { hook_event_name: 5 }]) assert.strictEqual(buildRecord(bad, 1), null);
});

test("se anota la carpeta del proyecto y desde dónde se usa Claude Code, sin rutas ni textos", () => {
  const win = buildRecord({ hook_event_name: "Stop", cwd: "C:\\Codigos\\tesela\\Tesela", prompt: "secreto" }, 5, null, { CLAUDE_CODE_ENTRYPOINT: "claude-vscode" });
  assert.deepStrictEqual(win, { event: "Stop", at: 5, project: "Tesela", origin: "claude-vscode" });
  assert.strictEqual(buildRecord({ hook_event_name: "Stop", cwd: "/home/e/app/" }, 5, null, {}).project, "app");
  assert.strictEqual("origin" in buildRecord({ hook_event_name: "Stop" }, 5, null, { CLAUDE_CODE_ENTRYPOINT: "../mal" }), false);
});

test("los subagentes se anotan al trabajar, se quitan al terminar y se vacían al cerrar el turno", () => {
  const dir = tmp();
  const file = path.join(dir, "sessions", "s9.json");
  const agents = () => JSON.parse(fs.readFileSync(file, "utf8")).agents;
  apply({ session_id: "s9", hook_event_name: "UserPromptSubmit" }, dir, 100, {});
  assert.strictEqual(agents(), undefined);
  apply({ session_id: "s9", hook_event_name: "SubagentStart", agent_id: "a1", agent_type: "Explore" }, dir, 101, {});
  apply({ session_id: "s9", hook_event_name: "PreToolUse", agent_id: "a2", agent_type: "security-reviewer" }, dir, 102, {});
  apply({ session_id: "s9", hook_event_name: "PreToolUse", tool_name: "Bash" }, dir, 103, {});
  assert.deepStrictEqual(agents(), [{ id: "a1", type: "Explore", at: 101 }, { id: "a2", type: "security-reviewer", at: 102 }]);
  apply({ session_id: "s9", hook_event_name: "PostToolUse", agent_id: "a1", agent_type: "Explore" }, dir, 110, {});
  apply({ session_id: "s9", hook_event_name: "SubagentStop", agent_id: "a2", agent_type: "security-reviewer" }, dir, 111, {});
  assert.deepStrictEqual(agents(), [{ id: "a1", type: "Explore", at: 110 }]);
  apply({ session_id: "s9", hook_event_name: "Notification", notification_type: "permission_prompt" }, dir, 112, {});
  assert.strictEqual(agents().length, 1, "un aviso ajeno no los borra");
  apply({ session_id: "s9", hook_event_name: "Stop" }, dir, 120, {});
  assert.strictEqual(agents(), undefined);
  apply({ session_id: "s9", hook_event_name: "SubagentStart", agent_id: "a3", agent_type: "Plan" }, dir, 130, {});
  apply({ session_id: "s9", hook_event_name: "PreToolUse" }, dir, 130 + 301, {});
  assert.strictEqual(agents(), undefined, "sin avisos en 5 minutos se da por terminado");
});

test("cada aviso reescribe el archivo de su sesión y SessionEnd lo borra", () => {
  const dir = tmp();
  assert.ok(apply({ session_id: "s1", hook_event_name: "UserPromptSubmit" }, dir, 100));
  const file = path.join(dir, "sessions", "s1.json");
  assert.deepStrictEqual(JSON.parse(fs.readFileSync(file, "utf8")), { event: "UserPromptSubmit", at: 100 });
  apply({ session_id: "s1", hook_event_name: "Stop" }, dir, 105);
  assert.deepStrictEqual(JSON.parse(fs.readFileSync(file, "utf8")), { event: "Stop", at: 105 });
  assert.deepStrictEqual(fs.readdirSync(path.join(dir, "sessions")), ["s1.json"], "sin archivos temporales");
  apply({ session_id: "s1", hook_event_name: "SessionEnd" }, dir, 110);
  assert.strictEqual(fs.existsSync(file), false);
  assert.ok(apply({ session_id: "s1", hook_event_name: "SessionEnd" }, dir, 111), "borrar lo que no existe no falla");
});

test("un identificador raro o un aviso sin evento no escriben nada", () => {
  const dir = tmp();
  assert.strictEqual(apply({ session_id: "../fuera", hook_event_name: "Stop" }, dir, 1), false);
  assert.strictEqual(apply({ session_id: "ok", hook_event_name: 7 }, dir, 1), false);
  assert.strictEqual(fs.existsSync(path.join(dir, "sessions")), false);
});

function run(input, dir) {
  return spawnSync(process.execPath, [SCRIPT], { input, env: { ...process.env, TESELA_CLAUDE_DIR: dir }, encoding: "utf8", timeout: 10000 });
}

test("como programa: lee stdin, no imprime nada y sale con 0 aunque la entrada esté rota", () => {
  const dir = tmp();
  const ok = run(JSON.stringify({ session_id: "s2", hook_event_name: "Notification", notification_type: "permission_prompt" }), dir);
  assert.strictEqual(ok.status, 0);
  assert.strictEqual(ok.stdout, "");
  assert.strictEqual(ok.stderr, "");
  assert.strictEqual(JSON.parse(fs.readFileSync(path.join(dir, "sessions", "s2.json"), "utf8")).notification_type, "permission_prompt");
  for (const input of ["", "no es json", "{", "[]", "null"]) {
    const r = run(input, dir);
    assert.strictEqual(r.status, 0, JSON.stringify(input));
    assert.strictEqual(r.stdout + r.stderr, "");
  }
});

test("como programa: con una carpeta imposible de crear sigue saliendo con 0", () => {
  const dir = tmp();
  const blocker = path.join(dir, "archivo");
  fs.writeFileSync(blocker, "x");
  const r = run(JSON.stringify({ session_id: "s3", hook_event_name: "Stop" }), path.join(blocker, "dentro"));
  assert.strictEqual(r.status, 0);
  assert.strictEqual(r.stdout + r.stderr, "");
});
