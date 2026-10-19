# Instalación, arranque con Windows y actualizaciones (Fase 10)

Todo lo de aquí está **preparado**; no se ha publicado, firmado ni instalado nada. (Para entender cómo funciona la app por dentro, ver [`como-funciona.md`](como-funciona.md).)

## 1. Generar el instalador (un solo archivo)

```powershell
npm install
npm run tauri build          # genera el instalador NSIS (usa la configuración de src-tauri/tauri.conf.json)
```

Resultado: `src-tauri\target\release\bundle\nsis\Tesela_1.7.0_x64-setup.exe`.

- Instala **por usuario**, sin permisos de administrador (`installMode: currentUser`) y en español.
- Incluye la carpeta `modules\` (notas, apps, portapapeles, velocidad, pareja y el ejemplo) en los recursos de la app; la
  pestaña **Módulos** muestra dónde quedó para instalarlos con un par de clics.
- Sin firmar: Windows SmartScreen mostrará «editor desconocido». Para evitarlo hace falta un certificado de firma de código
  (distinto de la clave del actualizador de abajo).
- La primera vez, `tauri build` descarga NSIS a la caché de Tauri (`%LOCALAPPDATA%\tauri`). **Yo no lo ejecuté** para no instalar
  herramientas en tu equipo sin pedírtelo.
- Para comprobar el programa de producción sin instalador: `cargo build --release --features custom-protocol` (la interfaz
  va incrustada y no necesita servidor de desarrollo). Esta compilación sí se hizo y se probó (ver PROGRESS.md).

## 2. Arranque con Windows

Pestaña **General** → «Iniciar la app al entrar en Windows». Escribe un valor en
`HKCU\Software\Microsoft\Windows\CurrentVersion\Run` (sin administrador) con la línea
`"<ruta>\tesela.exe" --manage all --bar`. Para quitarlo: desmarcar la casilla, o
`reg delete HKCU\Software\Microsoft\Windows\CurrentVersion\Run /v Tesela /f`.

**Importante:** no está activado en tu equipo. Y **desactiva el arranque de GlazeWM antes** (sin desinstalarlo): dos gestores a la
vez se pelean por las ventanas y por los atajos (GlazeWM ya tiene registrados `alt+…`).

## 3. Pasar de GlazeWM a esta app

1. Cierra GlazeWM (`alt+shift+e` en GlazeWM) y quítalo del arranque; **no lo desinstales**, es tu respaldo.
2. Abre la app → pestaña **General** → «Activar gestor de ventanas y barra…» (te avisa de que reorganizará todas las ventanas).
3. Si algo va mal: `Ctrl+Alt+Shift+F12` devuelve todas las ventanas a su sitio y pausa el gestor. Si la app se cerró de golpe y
   quedaron ventanas ocultas: `tesela.exe --recover`.
4. Para volver a GlazeWM: cierra esta app y ábrelo.

## 4. Actualizaciones (no conectadas todavía)

La app trae el código listo detrás de una opción de compilación (`--features updater`, plugin oficial de Tauri). Falta lo que
solo tú puedes poner:

1. **Clave de firma del actualizador** (secreta, fuera del repositorio):
   `npx tauri signer generate -w "$env:USERPROFILE\.tauri\tesela.key"`
2. En `src-tauri/tauri.conf.json` añadir:
   ```json
   "bundle": { "createUpdaterArtifacts": true },
   "plugins": { "updater": { "pubkey": "<contenido del .pub>", "endpoints": ["https://github.com/<usuario>/<repo>/releases/latest/download/latest.json"] } }
   ```
3. Compilar con la clave: `$env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content ...)` y `npm run tauri build -- --features updater`.
4. Publicar en GitHub Releases el instalador, su `.sig` y un `latest.json`:
   ```json
   { "version": "0.2.0", "notes": "…", "pub_date": "2026-01-01T00:00:00Z",
     "platforms": { "windows-x86_64": { "signature": "<contenido del .sig>", "url": "https://…/Window.Manager_0.2.0_x64-setup.exe" } } }
   ```
5. Pestaña **General** → «Buscar actualizaciones».

Sin la opción `updater`, la pestaña General dice claramente que las actualizaciones no están incluidas.

## 5. Lo que sí está probado

- El arranque con Windows se activa, consulta y desactiva con el registro real en una clave **de pruebas** propia (la real no se toca).
- La compilación de producción (`--features custom-protocol`) arranca con la interfaz incrustada, sin servidor de desarrollo, y pasa
  la prueba de extremo a extremo de módulos (ver PROGRESS.md).
- La configuración del instalador es válida para el compilador de Tauri (la compilación la procesa).
