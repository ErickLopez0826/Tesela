# Tesela: cómo funciona (visión teórica)

Este documento explica **qué es** Tesela, **cómo está organizada** y **por qué** funciona como funciona. No hace falta
saber programar para seguir las primeras secciones; las últimas bajan al detalle técnico. Para el historial de decisiones
concretas, ver [`DECISIONS.md`](../DECISIONS.md); para instalar y actualizar, [`instalacion-y-actualizaciones.md`](instalacion-y-actualizaciones.md);
para crear módulos, [`modules/README.md`](../modules/README.md).

---

## 1. Qué es Tesela

Tesela es una sola aplicación de escritorio para **Windows 11** que sustituye a varias cosas a la vez:

| Antes | Ahora |
|---|---|
| GlazeWM (gestor de ventanas en mosaico) | El **gestor de ventanas** de Tesela |
| Zebar (barra superior) | La **barra superior** de Tesela, una por monitor |
| Panel de control rápido de Windows | El **panel informativo** desplegable (medios, volumen, red, estadísticas, notas…) |
| Barra de tareas y menú Inicio de Windows | La **barra de tareas flotante** de Tesela y la tecla Windows que abre el panel |

Lo que hace, en una frase: **ordena las ventanas en columnas sin que el usuario tenga que arrastrarlas, agrupa el
escritorio en 9 espacios de trabajo (workspaces) por monitor, y ofrece alrededor una barra, un panel y una barra de
tareas con todo lo que se necesita a mano.**

Principios de diseño que explican casi todo lo demás:

1. **El «cerebro» no sabe nada de Windows.** Las reglas (dónde va cada ventana, qué workspace está activo…) están en
   código puro que se puede probar sin abrir una sola ventana real.
2. **Todo lo que toca Windows está aislado** en una capa de adaptadores. Si algo falla en Windows, se arregla ahí.
3. **Seguridad por defecto.** Las acciones que cambian el sistema (apagar, reiniciar, encender el Wi-Fi…) se simulan
   salvo que el usuario lo permita; los módulos de terceros solo hacen lo que el usuario les concede.
4. **Siempre se puede volver atrás.** Al cerrar Tesela (o con un modo de recuperación) las ventanas vuelven a su sitio, la
   barra de tareas de Windows reaparece y la tecla Windows vuelve a ser la de siempre.

---

## 2. Tecnologías

| Pieza | Para qué |
|---|---|
| **Tauri 2** | Marco de la app: una aplicación nativa que muestra interfaces web. Reúne el núcleo en Rust y las ventanas web. |
| **Rust** | El núcleo: reglas de ventanas, acceso a Windows, almacenamiento, comandos. |
| **Svelte 5** (TypeScript) | Las interfaces: barra, panel, barra de tareas, borde, ventana emergente. |
| **WebView2** | El navegador de Windows que dibuja esas interfaces dentro de cada ventana. |
| **`windows-rs`** | Acceso a la API de Windows (Win32, WinRT, Core Audio, registro…). |
| **`sysinfo`** | Lectura de procesos, memoria y CPU. |
| **Vitest / `cargo test`** | Pruebas del frontend y del núcleo. |

Detalle importante: Tesela **no es una ventana**, son **muchas ventanas pequeñas** (una barra por monitor, el panel, una
barra de tareas por monitor, el borde de la ventana activa, una ventana emergente…) que comparten **un único proceso y un
único núcleo**. Cada una es una página web distinta (`index.html?view=…`) que se comunica con el núcleo mediante
*comandos* y *eventos* de Tauri.

---

## 3. Arquitectura por capas

```
┌───────────────────────────────────────────────────────────────────────────┐
│ interface        Comandos de Tauri, ventanas propias, bandeja, protocolos │
│                  (bar, panel, taskbar, popup, shell, focus_border, tray…) │
├───────────────────────────────────────────────────────────────────────────┤
│ infrastructure   Adaptadores: Win32 (ventanas, hooks, iconos, registro),  │
│                  módulos de sistema (audio, red, energía…), archivos      │
├───────────────────────────────────────────────────────────────────────────┤
│ application      Controlador, puertos (traits), API de módulos, canales   │
├───────────────────────────────────────────────────────────────────────────┤
│ domain           Reglas puras: Desktop, layout, reglas, atajos, módulos   │
└───────────────────────────────────────────────────────────────────────────┘
          Las flechas de dependencia van siempre hacia abajo.
```

Código en `src-tauri/src/`:

- **`domain/`** — datos y reglas puras, sin Windows ni disco.
  `desktop.rs` (monitores, 9 workspaces, estados de ventana), `layout.rs` (motor de columnas), `rules.rs`
  (exclusiones y reglas por aplicación), `manageable.rs` (¿se puede gestionar esta ventana?), `shortcuts.rs` y
  `actions.rs` (atajos y sus acciones), `effects.rs` (lo que el escritorio pide hacer), `snapshot.rs` (el estado que se
  publica a las interfaces), `startup.rs` (apps de arranque), `shell.rs` (ajustes de la interfaz de Windows),
  `modules.rs` (manifiestos y permisos), `system.rs`/`utility.rs`/`tasks.rs` (tipos de los módulos de sistema).
- **`application/`** — orquesta el dominio.
  `controller.rs` (el controlador), `ports.rs` (la interfaz `WindowPlatform` que Windows debe implementar),
  `modules.rs` (API del núcleo para módulos y comprobación de permisos), `channels.rs` (mensajes entre módulos y
  equipos), `system.rs` (puertos de los módulos de sistema y el «modo seguro»), `list_windows.rs`.
- **`infrastructure/`** — el mundo real.
  `windows/` (todo lo que habla con Windows: `platform.rs`, `runtime.rs`, `facts.rs`, `enumerator.rs`, `appbar.rs`,
  `backdrop.rs`, `icons.rs`, `winkey.rs`, `winshell.rs`, `fade.rs`, `slide.rs`, `autostart.rs`, `startup_launch.rs`…),
  `windows/system/` (medios, volumen, red, energía, fondo de pantalla, procesos, portapapeles, segundo plano…) y los
  almacenes en disco (`config_file.rs`, `exclusion_store.rs`, `notes_store.rs`, `tasks_store.rs`, `startup_store.rs`,
  `shell_store.rs`, `module_store.rs`).
- **`interface/`** — lo que ve el usuario.
  `commands.rs` (todos los comandos que las páginas web pueden llamar), `bar.rs` (barra y panel), `taskbar.rs`
  (barra de tareas), `popup.rs` (ventana emergente de acciones), `shell.rs` (reemplazo de la interfaz de Windows),
  `focus_border.rs` (borde de la ventana activa), `tray.rs` (icono de bandeja), `modules_protocol.rs` (protocolo para
  servir módulos), `updates.rs` (actualizaciones), `webview.rs`.
- **`lib.rs`** arma todo: crea el estado, registra los comandos, abre las ventanas y arranca el gestor.

Frontend en `src/`: `lib/views/` (una por tipo de ventana), `lib/components/` (piezas visuales y sus pruebas),
`lib/stores/` (estado y utilidades: tema, sondeos, notas, tareas, barra de tareas…), `lib/api/` (las llamadas al núcleo).

### 3.1 El flujo de un evento

Cuando Windows abre una ventana:

```
Windows ──(hook de eventos)──► runtime.rs ──► Message::Opened(id)
      controlador: pide los «hechos» de la ventana (facts_of) a la plataforma
      domain::rules + manageable: ¿se gestiona? ¿a qué workspace va?
      Desktop::… devuelve una lista de Effect (Place, Hide, Show, Focus…)
      controlador: aplica cada Effect con platform.apply(...)  ──► Win32
      controlador: publica el estado (StateSnapshot) ──► barra, panel y barra de tareas
```

El mismo recorrido sirve para un atajo: el hook de teclas convierte `Alt+H` en `Message::Action(FocusLeft)` y el resto es
idéntico. Gracias a eso, **casi toda la lógica se prueba con una plataforma falsa** (`FakePlatform`) y sin tocar el
escritorio.

---

## 4. El gestor de ventanas

**Monitores y workspaces.** Cada monitor tiene su propio conjunto de workspaces (hasta 9). Solo uno por monitor está
visible. Cambiar de workspace oculta las ventanas del anterior y muestra las del nuevo, con una animación en la que las
ventanas salientes suben y las entrantes bajan (`slide.rs`, con miniaturas de DWM).

**Motor de columnas** (`layout.rs`). Las ventanas de un workspace se reparten en **grupos** (columnas); dentro de un
grupo pueden apilarse. El espacio se reparte de forma equitativa y respeta mínimos. El motor es una función pura: recibe
el área útil y las ventanas, y devuelve un rectángulo para cada una.

**Qué se gestiona y qué no.** No toda ventana debe entrar al mosaico (menús, avisos, el propio Tesela, programas
excluidos). `manageable.rs` descarta lo evidente (ventanas sin título, de herramientas, del shell) y `rules.rs` aplica
las **exclusiones** del usuario (editables desde el panel) y las **reglas por aplicación** (por ejemplo, que cierta aplicación se abra siempre
en un workspace concreto).

**Estados especiales.** Una ventana puede estar minimizada, en pantalla completa o flotante. En **pantalla completa**,
Tesela deja de ocupar el monitor: oculta la barra de ese monitor, la barra de tareas y se aparta; al salir, todo vuelve a su
sitio. Los juegos se detectan y no se tocan.

**Atajos y acciones.** `config.toml` define los atajos (por ejemplo `alt+h` para enfocar a la izquierda). Se registran
como atajos globales de Windows; cada uno es una `Action` (pura) que el controlador interpreta.

**Aplicaciones de arranque.** Una lista de programas que se abren con Tesela y se llevan a su workspace. Como muchas
aplicaciones crean su ventana tarde o sin título, el controlador las busca durante un rato tras lanzarlas y también mueve las
que ya estaban abiertas.

**Borde de la ventana activa.** Una ventana transparente que ignora el ratón se coloca alrededor de la ventana en uso y
dibuja el contorno (blanco, con un destello). Su color sigue al del fondo de cada monitor.

**Seguridad ante fallos.** Si algo va mal: un atajo de **emergencia** pausa la gestión y restaura todo; al cerrar la app,
`restore_all` devuelve cada ventana a su posición original; y las ventanas ocultas se anotan en disco para recuperarlas
tras un cierre brusco.

---

## 5. Las ventanas propias de Tesela

Todas son ventanas web transparentes, sin marco, y **sin foco** salvo el panel (ver 6.2 para el porqué).

### 5.1 Barra superior
Una por monitor, acoplada como **AppBar** (Windows reserva su franja y las demás ventanas no se le encimean). Muestra
workspaces, ventana activa, medios, reloj, etc. Se oculta si una app está en pantalla completa en ese monitor.

### 5.2 Panel informativo
Se despliega bajo la barra (con la tecla Windows o desde la barra). Tiene pestañas: **Controles** (medios, volumen,
conectividad), **Hoy**, **Estadísticas**, **Notas** y tareas, **Fondos** (incluye Wallpaper Engine), **Personalización**,
**Ajustes** (general, exclusiones, interfaz de Windows) y **Módulos**. Se cierra al perder el foco y se anima al abrirse y
cerrarse; la ventana se reutiliza y su WebView se «duerme» cuando está oculta para no gastar CPU.

### 5.3 Barra de tareas propia
Una **isla** flotante centrada abajo, **por encima de las aplicaciones** (no les quita espacio). Aparece al pulsar la
tecla Windows o al acercar el cursor al centro del borde inferior, y se oculta igual que el panel. Contiene:
- Las **apps abiertas agrupadas por workspace** (1, 2, 3…) con su icono; pulsar una la enfoca.
- Un botón de **aplicaciones en segundo plano** con contador: despliega **hacia arriba** una lista con las apps de la
  bandeja de Windows (Wallpaper Engine, Discord…), su CPU y su memoria. Cada una tiene «⋯», que abre una **ventana
  emergente** con acciones propias (ver 8.4).
- El botón de **energía**: al pulsarlo, la barra muestra solo Suspender, Reiniciar, Hibernar, Apagar (con confirmación) y
  «Volver».

El tamaño de la isla lo decide su contenido: la vista mide y pide al núcleo el tamaño; la ventana crece **hacia arriba**
anclada abajo.

### 5.4 Ventana emergente
Una única ventana (`popup`) que se coloca junto al botón «⋯» de una tarjeta de segundo plano y lista las acciones de esa app.
No ensancha la barra, no toma el foco y se cierra al actuar, al volver a pulsar el botón, al cerrarse la barra o al hacer
clic fuera (un hilo observa el botón del ratón mientras está abierta).

### 5.5 Animaciones coherentes
Panel, barra de tareas y ventana emergente aparecen en **0,5 s** y desaparecen en **0,38 s**; la ventana se oculta del todo
a los 460 ms. La salida usa animaciones propias (reutilizar la de entrada «al revés» no se reiniciaba).

---

## 6. Reemplazar la interfaz de Windows

Es la parte más delicada. Todo es **opcional** (casillas en Ajustes) y reversible.

### 6.1 Ocultar la barra de tareas de Windows
`winshell.rs` oculta `Shell_TrayWnd` (y las de otros monitores) y las pone en «ocultar automáticamente». Un hilo vigila
que Explorer no las vuelva a mostrar. Como la ocultación es peligrosa si la app muere, se deja una **marca en disco**; al
arrancar (o con `--recover`) la barra de Windows se devuelve si quedó oculta.

### 6.2 La tecla Windows
`winkey.rs` instala un **hook de teclado de bajo nivel** que:
1. **Retiene** la pulsación de Windows (Windows no la ve).
2. Si llega otra tecla mientras está pulsada (Win+Shift+S, Win+V…), entrega primero la pulsación retenida: **los atajos
   con Windows siguen funcionando**.
3. Si se suelta sin otra tecla, es un **toque**: se traga y se abre o cierra el panel y la barra de tareas. Windows nunca
   llega a abrir Inicio ni la búsqueda.

La acción de cada toque corre en un **hilo aparte**: un hook que tarda más de ~300 ms hace que Windows ignore lo que devuelve.

**El problema del «gancho sordo».** Descubierto con pruebas reproducibles: cuando el WebView del panel recibe un clic y
toma el foco de teclado, Windows **deja de llamar a todos los hooks de teclado** (también a los de otros procesos). Entonces
la tecla Windows llega sin filtrar y se abriría la búsqueda. Dos defensas:
- El panel se abre **sin activarse** y el hook se reinstala en un hilo nuevo cada pocos cientos de ms mientras está abierto.
- **Respaldo desde la propia página:** el panel avisa al núcleo de las teclas que le llegan; al pulsar Windows se inyecta
  una tecla neutra (Windows lo ve como un atajo y no abre nada) y al soltarla sin otra tecla cuenta como toque.

Además, un **vigilante de foco** (evento del sistema, no sondeo) oculta y cierra con Escape cualquier Inicio o Búsqueda que
llegue a colarse, por si acaso.

### 6.3 Qué no se puede
Los iconos de la **bandeja del sistema** no tienen API pública. Tesela usa el registro de Windows 11
(`HKCU\Control Panel\NotifyIconSettings`, donde Windows recuerda cada app con icono) cruzado con los procesos en marcha
para mostrar las aplicaciones en segundo plano; no reproduce los menús propios de cada icono.

---

## 7. Apariencia

- **Tema y color.** Un color de acento global (manual o «adaptado al fondo», **por monitor**: se calcula a partir del
  fondo de pantalla de cada uno). Los cambios de color se animan como una **ola de puntos** que parte del clic o, al
  cambiar el fondo, del centro.
- **Cristal (blur).** La barra y el panel copian lo que hay detrás (`backdrop.rs`), lo desenfocan con la intensidad de
  Personalización y lo muestran bajo un tinte. La barra de tareas copia una zona fija del tamaño máximo de la isla y la
  recoloca según su tamaño actual, para que el desenfoque no desaparezca al crecer.
- **Texto dinámico.** El color del texto del panel se elige según la luminosidad de lo que hay detrás (blanco sobre fondo
  oscuro y viceversa).
- **Transiciones del sistema.** Al cambiar el fondo, `fade.rs` dibuja una transición propia («grow», como Hyprland);
  `slide.rs` anima el cambio de workspace.
- **Ahorro con la interfaz oculta.** Una ventana oculta no consulta nada ni anima: los sondeos y el WebView se detienen mientras no se ve.

---

## 8. Módulos

### 8.1 Módulos de sistema (parte del núcleo)
Acceden a Windows con adaptadores en `infrastructure/windows/system/`:
**medios** (sesión multimedia de Windows), **volumen** y dispositivos de audio (Core Audio), **conectividad** (Wi-Fi,
Bluetooth, zona compartida y estado de red), **energía**, **estadísticas** (CPU, RAM, GPU, discos, procesos que más
consumen), **fondo de pantalla** y **Wallpaper Engine**, **aplicaciones instaladas/abiertas**, **portapapeles**, **prueba de
velocidad**, y la lectura de **aplicaciones en segundo plano** y su audio por proceso.

**Modo seguro.** Las acciones que cambian el sistema pasan por un envoltorio (`Guarded`): con `allow_changes = false`
solo se simulan y devuelven «Simulado: se habría…». Las lecturas siempre funcionan.

### 8.2 Notas y tareas
Se guardan en JSON en la carpeta de configuración. Las notas se pueden exportar (una, varias o todas) y filtrar por fechas
(con un selector de día, mes y año). Las tareas se completan con una animación de salida.

### 8.3 Módulos instalables
Un **módulo** es una carpeta con `module.json` y una página web (HTML+JS+CSS) que se muestra en un **iframe aislado**: no ve
Windows ni Tauri. Pide al núcleo, por nombre, operaciones de alto nivel y el núcleo decide según los **permisos** que el
usuario concedió al instalarlo (leer estado, enviar mensajes a un canal, red, archivos propios…).
- El protocolo `wmmodule://` sirve sus archivos con una **política de seguridad (CSP)** calculada según los permisos.
- El **SDK** (`__wm/sdk.js`) es lo único que el módulo carga del núcleo.
- Los **canales** (`channels.rs`) permiten que módulos (y equipos) se envíen mensajes; hoy hay un canal local y la
  preparación para uno remoto.
- Incluidos: ejemplo `hello`, **velocidad** y **pareja** (mensajes entre dos equipos; documentado en `modulo-pareja.md`).
- Se instalan y desinstalan sin dejar restos (`module_store.rs`).

### 8.4 Acciones por aplicación en segundo plano
Para cada app de la bandeja en marcha, el núcleo ofrece (`app_actions`):
- **Abrir** (muestra y enfoca su ventana aunque esté oculta; si no tiene, la lanza) y **Cerrar** (con confirmación).
- **Silenciar app** si tiene sesión de audio (control de volumen por proceso, sin tocar el volumen general).
- **Acciones de su perfil** (`app_actions.rs`): Discord (atajos globales que el usuario define igual en Discord), Wallpaper
  Engine (`-control pause/play/mute/unmute`), Spotify (teclas multimedia), Steam (enlaces `steam://`). Se pueden
  añadir o reemplazar perfiles en `app-actions.json` (`keys`, `exec` o `uri`).

**Regla de seguridad:** la interfaz solo manda el nombre del ejecutable y el id de la acción. La ruta, los argumentos y los
enlaces los resuelve el núcleo contra su propia lista de aplicaciones en marcha; así una página comprometida no puede lanzar
programas arbitrarios.

---

## 9. Datos, configuración y archivos

| Qué | Dónde |
|---|---|
| Configuración principal (layout, workspaces, reglas, atajos, modo seguro) | `%APPDATA%\Tesela\config.toml` |
| Exclusiones, notas, tareas, apps de arranque | `exclusions.json`, `notes.json`, `tasks.json`, `startup-apps.json` (misma carpeta) |
| Interfaz de Windows (`replace_taskbar`, `win_key_panel`, `show_background`) | `shell.json` |
| Perfiles de acciones por app | `app-actions.json` |
| Módulos instalados | `modules.json` y la carpeta de cada módulo |
| Estado, marca de «barra de Windows oculta», ventanas ocultas, registro de la tecla Windows | `%LOCALAPPDATA%\Tesela\state\` (`winkey.log`…) |
| Programa instalado | `%LOCALAPPDATA%\Tesela\` |

Si un archivo falta o está dañado, se usan los valores de siempre; nada rompe la app.

---

## 10. Compilar, versiones y pruebas

- **Compilar el instalador:** `npm run tauri build` genera un instalador NSIS por usuario (sin administrador, en español).
- **Versiones** (`MAYOR.MENOR.PARCHE`): el primer número solo cambia cuando el usuario lo decide (un cambio completo de la
  app); el segundo, con cambios significativos (un módulo nuevo o una modificación grande); el tercero, con arreglos,
  rendimiento o cambios pequeños.
- **Actualizaciones:** preparadas tras una opción (`updates.rs`, `--features updater`); no están conectadas a un servidor.
- **Pruebas:**
  - Frontend: `npx vitest run` (componentes y utilidades, con jsdom).
  - Núcleo: `cargo test --lib` (reglas puras y controlador con plataforma falsa).
  - **En vivo** (`cargo test --test live_shell -- --ignored`): tocan el escritorio real (ocultar la barra, leer la bandeja, pulsar
    la tecla Windows) y por eso están marcadas `#[ignore]`.
- **Diagnóstico:** `winkey.log` registra cada pulsación de Windows, qué decidió el gancho y los cambios de foco.
  `--recover` devuelve la barra de Windows si Tesela terminó mal.

---

## 11. Limitaciones conocidas

- La **bandeja** de Windows no se puede leer ni reproducir con exactitud (6.3); no se muestran sus menús.
- **Discord** no se puede controlar sin registrar una aplicación propia; se usa con atajos globales configurados a mano.
- Algunas ventanas con **más privilegios** (p. ej. el Administrador de tareas) no se pueden mover.
- Con un WebView de Tesela enfocado, Windows no llama a los hooks de teclado; hay respaldo, pero solo para el panel.
- La **ventana emergente** y la barra de tareas usan estilos distintos al panel (sin desenfoque en el popup).
- Acciones de **energía** y conectividad: se verificaron sobre todo en modo seguro (simuladas).

## Glosario

- **Hook**: función que Windows llama ante un evento (una tecla, una ventana nueva).
- **AppBar**: ventana que reserva una franja del monitor.
- **Workspace**: espacio de trabajo; una «pantalla virtual» con sus propias ventanas.
- **Effect**: orden que el dominio puro devuelve («coloca esta ventana aquí»); lo ejecuta la plataforma.
- **Snapshot**: foto del estado visible que se publica a las interfaces.
- **WebView2**: el navegador embebido que dibuja cada ventana de Tesela.
- **Modo seguro**: simulación de las acciones que cambian el sistema.
