# Módulos de Window Manager

Un módulo es una carpeta con un `module.json` y una página web (HTML + JS + CSS). La app la muestra en un
**iframe aislado**: no ve Windows, no ve Tauri y no puede salir de su carpeta. Todo lo que quiera del sistema lo
pide a la **API del núcleo**, y el núcleo decide según los permisos que el usuario concedió al instalar.

## Estructura

```
mi-modulo/
├─ module.json     manifiesto (obligatorio)
├─ index.html      página de entrada (campo "entry")
├─ app.js          tu código (sin scripts en línea: la política de seguridad los prohíbe)
└─ style.css
```

En `index.html` carga el SDK que sirve el núcleo (no lo copies) y después tu código:

```html
<script src="__wm/sdk.js"></script>
<script src="app.js"></script>
```

## module.json

| Campo | Qué es |
|---|---|
| `id` | minúsculas, números y guiones (máx. 40) |
| `name`, `version` (`x.y.z`), `author`, `description` | datos que se muestran al instalar |
| `min_core_version` | versión mínima de la app (`x.y.z`) |
| `permissions` | lista de permisos que pides (ver abajo) |
| `network_hosts` | si pides `network`: los servidores permitidos (`https://…` / `wss://…`, sin comodines) |
| `dependencies` | `[{ "id": "otro-modulo", "min_version": "1.0.0" }]` |
| `entry` | página de entrada (por defecto `index.html`) |
| `ui` | `{ "panel": { "title", "height" } }` y/o `{ "bar": { "width" } }`: dónde aparece tu módulo |
| `actions` | `[{ "id", "title" }]`: botones que aparecen bajo tu sección del panel |
| `shortcuts` | atajos que sugieres para tus acciones (el usuario decide si los usa) |

## Permisos

| Permiso | Da acceso a |
|---|---|
| `windows` | `windows.active_workspace`, `windows.workspaces`, `windows.focused` y los eventos `workspace_changed`, `focus_changed` |
| `files` | `files.read/write/list/delete` **solo** dentro de tu carpeta de datos (máx. 1 MB por archivo, 100 archivos) |
| `notifications` | `notify.show` |
| `system` | `system.stats` (CPU y memoria) |
| `power` | `power.run` (sigue sujeto al modo seguro de la app: por defecto solo se simula) |
| `network` | conectarte a los `network_hosts` declarados (lo aplica la política de seguridad del iframe) |

Sin permiso, la llamada devuelve el error `permission_denied`.

## API (SDK)

```js
await wm.call("windows.active_workspace");            // { number: 2 }
await wm.call("files.write", { path: "a.txt", content: "hola" });
await wm.on("workspace_changed", (p) => console.log(p.number));
wm.onAction((id) => { if (id === "saludar") { /* … */ } });
```

Errores: el `Error` trae un código al principio del mensaje (`permission_denied`, `unknown_method`,
`bad_params`, `failed`, `disabled`). Un módulo que falla tres veces seguidas se desactiva solo.

## Probar

El módulo de ejemplo está en `examples/hello`. Instálalo desde la pestaña **Módulos** de la app indicando su carpeta:
verás los permisos que pide antes de aceptar. La prueba de extremo a extremo (`cargo test --test phase7_modules`)
instala, usa y desinstala módulos con la app real.
