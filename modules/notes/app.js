/* Módulo «Notas». Cada nota es un archivo .md o .txt en la carpeta del módulo; sin base de datos.
 * Solo usa la API del núcleo (`wm`) con el permiso "files". */

var NOTES_DIR = "notas/";
var EXTENSIONS = [".md", ".txt"];

/** Nombre de archivo seguro a partir de lo que escribió el usuario (sin rutas ni caracteres raros). */
function safeNoteName(raw, existing) {
  var base = String(raw || "").replace(/\.(md|txt)$/i, "").replace(/[^\p{L}\p{N} _-]/gu, "").trim().slice(0, 60);
  if (!base) base = "Nota";
  var name = base + ".md";
  var n = 2;
  while ((existing || []).indexOf(name) !== -1) {
    name = base + " " + n + ".md";
    n++;
  }
  return name;
}

function isNoteFile(path) {
  var p = path.toLowerCase();
  return p.indexOf(NOTES_DIR) === 0 && EXTENSIONS.some(function (e) { return p.slice(-e.length) === e; });
}

/** Markdown básico y SEGURO: se construye con nodos DOM y textContent, nunca con innerHTML. */
function renderMarkdown(text, doc) {
  var out = doc.createDocumentFragment();
  var lines = String(text).split(/\r?\n/);
  var i = 0;
  function inline(parent, s) {
    var re = /(\*\*[^*]+\*\*|\*[^*]+\*|`[^`]+`)/g;
    var last = 0;
    var m;
    while ((m = re.exec(s))) {
      if (m.index > last) parent.appendChild(doc.createTextNode(s.slice(last, m.index)));
      var tok = m[0];
      var el;
      if (tok.slice(0, 2) === "**") { el = doc.createElement("strong"); el.textContent = tok.slice(2, -2); }
      else if (tok[0] === "`") { el = doc.createElement("code"); el.textContent = tok.slice(1, -1); }
      else { el = doc.createElement("em"); el.textContent = tok.slice(1, -1); }
      parent.appendChild(el);
      last = m.index + tok.length;
    }
    if (last < s.length) parent.appendChild(doc.createTextNode(s.slice(last)));
  }
  while (i < lines.length) {
    var line = lines[i];
    if (/^```/.test(line)) {
      var code = [];
      i++;
      while (i < lines.length && !/^```/.test(lines[i])) { code.push(lines[i]); i++; }
      i++;
      var pre = doc.createElement("pre");
      var c = doc.createElement("code");
      c.textContent = code.join("\n");
      pre.appendChild(c);
      out.appendChild(pre);
    } else if (/^#{1,3} /.test(line)) {
      var level = line.indexOf(" ");
      var h = doc.createElement("h" + level);
      inline(h, line.slice(level + 1));
      out.appendChild(h);
      i++;
    } else if (/^[-*] /.test(line)) {
      var ul = doc.createElement("ul");
      while (i < lines.length && /^[-*] /.test(lines[i])) {
        var li = doc.createElement("li");
        inline(li, lines[i].slice(2));
        ul.appendChild(li);
        i++;
      }
      out.appendChild(ul);
    } else if (line.trim() === "") {
      i++;
    } else {
      var p = doc.createElement("p");
      inline(p, line);
      out.appendChild(p);
      i++;
    }
  }
  return out;
}

async function start(wm, doc) {
  var el = {
    list: doc.getElementById("list"),
    editor: doc.getElementById("editor"),
    preview: doc.getElementById("preview"),
    status: doc.getElementById("status"),
    newBtn: doc.getElementById("new"),
    toggle: doc.getElementById("toggle"),
    del: doc.getElementById("delete"),
  };
  var state = { names: [], current: null, viewing: false, saveTimer: null, folder: "" };

  function setStatus(text, isError) {
    el.status.textContent = text;
    el.status.className = isError ? "error" : "";
  }

  async function refreshList() {
    var files = await wm.call("files.list");
    state.names = files.filter(isNoteFile).map(function (f) { return f.slice(NOTES_DIR.length); }).sort();
    el.list.textContent = "";
    state.names.forEach(function (n) {
      var o = doc.createElement("option");
      o.value = n;
      o.textContent = n.replace(/\.(md|txt)$/i, "");
      el.list.appendChild(o);
    });
    if (state.current) el.list.value = state.current;
  }

  async function open(name) {
    state.current = name;
    var r = await wm.call("files.read", { path: NOTES_DIR + name });
    el.editor.value = r ? r.content : "";
    el.list.value = name;
    renderPreview();
  }

  function renderPreview() {
    el.preview.textContent = "";
    el.preview.appendChild(renderMarkdown(el.editor.value, doc));
  }

  async function save() {
    if (!state.current) return;
    try {
      await wm.call("files.write", { path: NOTES_DIR + state.current, content: el.editor.value });
      setStatus("Guardado en " + (state.folder || "la carpeta de notas") + "\\" + state.current.replace(/\//g, "\\"), false);
    } catch (e) {
      setStatus("No se pudo guardar: " + String(e.message || e), true);
    }
  }

  async function create() {
    var name = safeNoteName("Nueva nota", state.names);
    await wm.call("files.write", { path: NOTES_DIR + name, content: "# " + name.replace(/\.md$/, "") + "\n\n" });
    await refreshList();
    await open(name);
    el.editor.focus();
  }

  async function remove() {
    if (!state.current) return;
    await wm.call("files.delete", { path: NOTES_DIR + state.current });
    state.current = null;
    await refreshList();
    if (state.names.length > 0) await open(state.names[0]);
    else { el.editor.value = ""; renderPreview(); setStatus("Sin notas. Pulsa «Nueva».", false); }
  }

  function setViewing(v) {
    state.viewing = v;
    el.editor.hidden = v;
    el.preview.hidden = !v;
    el.toggle.textContent = v ? "Editar" : "Vista";
    if (v) renderPreview();
  }

  el.editor.addEventListener("input", function () {
    clearTimeout(state.saveTimer);
    state.saveTimer = setTimeout(save, 500);
  });
  el.list.addEventListener("change", function () { save().then(function () { return open(el.list.value); }); });
  el.newBtn.addEventListener("click", function () { create().catch(showError); });
  el.del.addEventListener("click", function () { remove().catch(showError); });
  el.toggle.addEventListener("click", function () { setViewing(!state.viewing); });
  wm.onAction(function (id) { if (id === "nueva") create().catch(showError); });

  function showError(e) { setStatus(String(e.message || e), true); }

  try {
    var dir = await wm.call("files.dir");
    state.folder = dir && dir.path ? dir.path + "\\notas" : "";
    await refreshList();
    if (state.names.length === 0) {
      // Primera vez: una nota de bienvenida para que se vea cómo funciona.
      await wm.call("files.write", {
        path: NOTES_DIR + "Bienvenida.md",
        content: "# Bienvenida\n\nEscribe tus notas aquí. Se guardan solas como archivos en una carpeta local.\n\n- Soporta **negrita**, *cursiva* y `código`\n- Usa «Vista» para ver el formato\n",
      });
      await refreshList();
    }
    await open(state.names[0]);
    setStatus(state.folder ? "Carpeta: " + state.folder : "", false);
  } catch (e) {
    showError(e);
  }
}

if (typeof window !== "undefined" && window.wm) {
  start(window.wm, document);
}
