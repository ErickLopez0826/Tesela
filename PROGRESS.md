# PROGRESS

Estado por fase de la sección 12. Leyenda: ✅ cumplido · 🟡 parcial · ⬜ pendiente.

Verificación previa (estado inicial): PROGRESS.md no existía; solo estaba hecha la Fase 0. Las fases 1 y 2 no estaban implementadas y se hicieron desde cero.

## Resumen por fase (criterio de «Terminado cuando» de la sección 12)

| Fase | Criterio | Estado |
|---|---|---|
| 0 Base | La app lista las ventanas visibles con su ejecutable y su título | ✅ cumplido |
| 1 Núcleo de ventanas | Abrir, cerrar y cambiar entre ventanas sin que se pierdan ni queden fuera de pantalla | ✅ cumplido, **con una desviación**: ocultar por workspace usa `SW_HIDE` porque el cloak de DWM da «acceso denegado» (B-2) |
| 2 Motor de layout | Todas las reglas de la sección 5 pasan sus pruebas | ✅ cumplido |
| 3 Atajos y configuración | Cada atajo de la sección 8 ejecuta su acción | ✅ cumplido; los `alt+…` reales no se pudieron registrar mientras GlazeWM corre (B-3): se probó la misma lógica con otro modificador |
| 4 Exclusiones y pantalla completa | Un juego y una app excluida funcionan como si GlazeWM no existiera | ✅ cumplido con ventanas de prueba (no un juego real); alcances de §6.5 distintos de «siempre» pendientes (el plan los deja para después) |
| 5 Barra y panel | La barra reserva su espacio y no se encima con las ventanas | ✅ cumplido; contenido de la barra fijo, no configurable por zonas (B-6) |
| 6 Módulos de sistema y acción | Cada acción del panel funciona desde la interfaz | 🟡 **parcial**: todo lo no disruptivo verificado con el sistema real; energía, Wi-Fi, Bluetooth y compartir red **solo simulados** por tus reglas de seguridad (B-7) |
| 7 Módulos instalables | El módulo de prueba se instala, pide permisos, funciona y se desinstala limpiamente | ✅ cumplido (prueba real con la app, también con la compilación de producción) |
| 8 Módulos de utilidad | Cada módulo funciona como módulo instalable | ✅ cumplido; la prueba de velocidad usa datos de ejemplo hasta que instales el CLI de Ookla (B-9) |
| 9 Módulo de pareja | Dos instancias se envían mensajes con delay razonable | 🟡 **preparada, criterio NO cumplido entre equipos reales**: sin backend (necesita tu proyecto de Supabase, B-10); cumplido solo con el canal simulado |
| 10 Instalación | La app arranca sola, se instala con un solo archivo y se actualiza | 🟡 **preparada, criterio NO cumplido**: el arranque con Windows existe y está probado (clave de pruebas); el instalador está configurado pero **no generado**; las actualizaciones **no están conectadas** (B-11) |
| 11 Buscador de aplicaciones (2.0.0) | Erick puede desinstalar Flow Launcher y hacer con Tesela todo lo que hacía con él (§18.12) | 🟡 **código y pruebas hechos; falta tu prueba de uso real** (ver abajo) |

### Cómo ejecutar todas las pruebas
```
cd src-tauri
cargo test                                   # unitarias (dominio, aplicación, infraestructura): sin ventanas reales
cd .. && npx vitest run                      # interfaz y módulos
cd src-tauri
cargo test --test phase1_windows --test phase3_hotkeys --test phase4_exclusions --test phase5_bar --test phase6_services -- --ignored --nocapture --test-threads=1
cargo test --test phase7_modules --test phase8_utilities --test phase9_partner -- --ignored --nocapture --test-threads=1   # necesitan npm (o WM_TEST_EXE con la compilación de producción)
```
Las pruebas «reales» (`--ignored`) usan solo ventanas de prueba `WMTEST` (nunca tus ventanas), no cambian tu sistema y dejan GlazeWM intacto.

## Fase 0: Base ✅
- ✅ Proyecto Tauri con capas (domain / application / infrastructure / interface).
- ✅ Lista ventanas visibles con ejecutable y título.

## Fase 1: Núcleo de ventanas ✅ (con una desviación)
- ✅ Eventos de ventanas (abrir, cerrar, enfocar, minimizar, mover) con `SetWinEventHook`.
- ✅ Mover y redimensionar con `SetWindowPos`, compensando el borde invisible de DWM.
- 🟡 Ocultar/mostrar por workspace: **el cloak de DWM no funciona** para ventanas de otros procesos desde un proceso normal (`DwmSetWindowAttribute(DWMWA_CLOAK)` devuelve `E_ACCESSDENIED`, medido con `tests/cloak_probe.rs`). Se usa `SW_HIDE` como alternativa (D-8). Ver BLOCKERS.md B-2.
- ✅ Atajo de emergencia `Ctrl+Alt+Shift+F12` (probado con la tecla real inyectada).
- ✅ Restauración al salir, ante pánico del controlador y tras cierre brusco (`--recover`).
- ✅ Criterio "abrir, cerrar y cambiar entre varias ventanas sin que se pierdan ni queden fuera de pantalla": prueba unitaria con invariante + prueba real `tests/phase1_windows.rs` (ventanas WMTEST creadas por script).

Cómo ejecutar la prueba real: `cargo test --test phase1_windows -- --ignored --nocapture --test-threads=1` (desde `src-tauri`).

## Fase 2: Motor de layout ✅
- ✅ Modelo de columnas y ventanas apiladas (`domain::layout`), con ejes columnas/filas y apilado vertical/horizontal.
- ✅ Inserción a la derecha, reparto proporcional, mínimos ("llenado de agua"), cierre y movimiento (§5.1–5.4).
- ✅ Monitores verticales (§5.5, eje configurable, filas por defecto) y caso límite sin espacio (§5.6, flotante centrada).
- ✅ Estados de ventana gestionada/flotante/minimizada con vuelta a su posición (§7).
- ✅ Criterio "todas las reglas de la sección 5 pasan sus pruebas": cada caso de las secciones "Layout" y "Monitores" de §13 tiene su prueba en `domain::layout` y `domain::desktop` (Desktop con dos monitores, mover workspace entre monitores, desconectar monitor).
- Sin ventanas reales: el dominio se prueba con datos puros. La prueba real de la Fase 1 sigue pasando con el nuevo Desktop.

## Fase 3: Atajos y configuración ✅
- ✅ Atajos cargados desde `config.toml` editable (TOML); creado con valores por defecto si falta.
- ✅ Todos los atajos de la sección 8 migrados (58 globales + 10 del modo redimensionar). Una prueba compara el archivo por defecto contra la lista de la sección 8 escrita aparte (ni más ni menos).
- ✅ Recarga en caliente con `alt+shift+r`; con configuración inválida conserva la anterior y avisa (probado con teclas reales).
- ✅ Criterio "cada atajo de la sección 8 ejecuta su acción correcta": `cada_atajo_global_de_la_seccion_8_ejecuta_su_accion` pulsa cada combinación registrada en un escritorio de 3 monitores y comprueba que cambia el estado. Prueba real con teclas inyectadas en `tests/phase3_hotkeys.rs` (usa Ctrl+Alt+F<n> y F13/F14 para no chocar con GlazeWM, que sigue corriendo y ya tiene alt+...).
- 🟡 Excepción: `alt+f` (pantalla completa) se implementa en la Fase 4.
- Atajos con `alt+...` reales **no se pudieron probar contra el sistema** mientras GlazeWM siga corriendo (B-3); la lógica es idéntica porque solo cambia el texto de la combinación.

## Fase 4: Exclusiones y pantalla completa ✅ (con pendientes menores)
- ✅ Panel de exclusiones (Svelte): desde ventanas abiertas con un clic, por nombre de proceso o eligiendo un `.exe`, con título como criterio adicional; lista de excluidas con "Quitar". Comandos Rust `list/add/remove_exclusion`, persistencia en JSON y reevaluación en caliente de las ventanas abiertas.
- ✅ Comportamiento de excluida (§6.2): no se mueve, no cuenta para el reparto, no recibe atajos de posición, se vigila hasta que se cierra y no deja estado.
- ✅ Pantalla completa (§6.4): oculta la señal de barra del monitor correspondiente (gestionadas y excluidas); dos pantallas completas en monitores distintos se manejan por separado. La barra visual llega en la Fase 5; hoy la señal es el efecto `SetBar` / evento `wm-bar-visibility`.
- ✅ Criterio "un juego y una aplicación excluida se abren y funcionan como si GlazeWM no existiera": `tests/phase4_exclusions.rs` (real) abre dos ventanas gestionadas, una app excluida y un "juego" sin borde que se agranda a pantalla completa: ni se mueven ni cambian de tamaño, el reparto no cambia, la barra se oculta y reaparece. Limitación: usa ventanas WPF de prueba, no un juego de verdad ni aplicaciones de terceros.
- 🟡 Alcances de §6.5 distintos de "siempre": no implementados (el plan los marca como posteriores).

## Fase 5: Barra y panel ✅ (contenido fijo)
- ✅ Barra por monitor acoplada como AppBar (`--bar`): una ventana por monitor, alto escalado por DPI, se crea/retira al conectar o desconectar monitores y se oculta en pantalla completa.
- ✅ Panel desplegable (clic en ☰ o `alt+b`): reloj, CPU, RAM y los 9 workspaces con clic para cambiar.
- ✅ Criterio "la barra reserva su espacio y no se encima con las ventanas": `tests/phase5_bar.rs` arranca la app real con tus dos monitores (1920×1080 y 1080×1920 vertical): hay una barra por monitor, el área útil de cada uno quedó por debajo de la barra y las ventanas de prueba gestionadas quedan debajo de ella (y=60 = 40 de barra + 20 de margen). Al cerrar la app el área útil vuelve a la original.
- 🟡 Contenido de la barra no configurable por zonas todavía (B-6).

## Fase 6: Módulos de sistema y acción 🟡 (lectura verificada con el sistema real; escritura en modo seguro)
- ✅ Estadísticas (CPU, GPU, RAM, discos, red, tiempo encendido, apps que más consumen): lectura real verificada (`tests/phase6_services.rs`).
- ✅ Medios: lee lo que suena en Spotify (título, artista, estado) con SMTC; controles anterior/pausa/siguiente implementados.
- ✅ Volumen: lectura real, ajuste y silencio (probado solo reescribiendo el mismo valor). Barra: volumen, lo que suena y temperatura si existe.
- ✅ Red: velocidad de bajada/subida (la prueba de velocidad con Ookla es de la Fase 8).
- ✅ Energía: las cuatro acciones con confirmación en dos pasos. 🟡 **Solo simulada**: modo seguro por defecto (D-39); el adaptador real existe y está probado a nivel de qué comando ejecuta cada acción, pero no se ejecutó.
- ✅ Wi-Fi, Bluetooth y compartir red: estado real leído (Wi-Fi sí, Bluetooth no, compartir red disponible). 🟡 Cambiarlos solo está simulado en las pruebas.
- ✅ Fondos con transición: lista de imágenes de la carpeta y aplicación con `IDesktopWallpaper`; solo lectura del fondo actual verificada (B-7).
- 🟡 Criterio "cada acción del panel funciona desde la interfaz": cada acción tiene su botón, su comando Tauri y su adaptador, y la interfaz está probada con Vitest (47 pruebas); con el sistema real se verificó todo lo que no es disruptivo. Las acciones disruptivas funcionan en modo simulado y quedan para que las confirmes (B-7).
- Corrección encontrada al arrancar la app real: `sysinfo` inicializaba COM en el hilo principal y tumbaba la app (`RPC_E_CHANGED_MODE`); y las ventanas creadas desde el hilo principal bloqueaban el panel. Ambos corregidos (D-44).

## Fase 7: Sistema de módulos instalables ✅
- ✅ Manifiesto validado (§10.2): datos, versión mínima del núcleo, permisos, dependencias, componentes de interfaz (panel/barra), acciones y atajos sugeridos.
- ✅ Cargador: instalación atómica desde carpeta, registro de permisos concedidos, activar/desactivar, actualizar sin perder datos, desinstalación completa.
- ✅ Permisos (§10.3): se muestran antes de instalar; solo se entrega lo declarado **y** aceptado; revocables después.
- ✅ API del núcleo (§10.4): ventanas/workspaces, estadísticas, energía (con modo seguro), avisos, archivos propios, suscripción a eventos.
- ✅ Aislamiento: iframe en sandbox + CSP por permisos + protocolo propio; el módulo no toca Windows.
- ✅ Módulo de prueba (`modules/examples/hello`) y sonda de permisos.
- ✅ Criterio "el módulo de prueba se instala, pide permisos, funciona y se desinstala limpiamente": `tests/phase7_modules.rs` con la app REAL: (1) vista previa con 3 permisos y concesión explícita; (2) el módulo corre en su iframe, lee/escribe su contador en su carpeta, consulta el escritorio y se suscribe a eventos (segunda apertura → contador 2); (3) desinstalar borra archivos, datos y registro, y la app ya no lo encuentra. La sonda confirma que lo no concedido devuelve `permission_denied`, que no puede escapar de su carpeta de datos y que no obtiene respuesta de Tauri por la puerta de atrás.
- ✅ Casos de §13 "Módulos": permiso no concedido → no accede (real + unitaria); desinstalar no deja procesos ni suscripciones (unitaria: suscripciones y registro; un módulo no lanza procesos); un módulo que falla no tumba el núcleo (unitaria con pánico + real).
- 🟡 Ver B-8 (red solo por CSP, barra y acciones sin verse en la app real, instalar sin selector nativo).

## Fase 8: Módulos de utilidad ✅ (con el CLI de Ookla pendiente de instalar)
- ✅ **Notas**: Markdown o texto plano, un archivo por nota en una carpeta local; guardado automático; vista con formato segura (sin HTML).
- ✅ **Apps abiertas y fijadas**: dos listas intercambiables; las abiertas se actualizan en tiempo real, incluso las ocultas por estar en otro workspace; ir a una, fijarla, abrir una fijada (o saltar a ella si ya está abierta).
- ✅ **Portapapeles**: historial, un clic para volver a copiar, hueco en la barra, respeta a los gestores de contraseñas.
- ✅ **Prueba de velocidad**: inicia en segundo plano, consulta el estado, guarda resultados. 🟡 **Con datos de ejemplo hasta que instales el CLI de Ookla** (B-9).
- ✅ Criterio "cada módulo funciona como módulo instalable": `tests/phase8_utilities.rs` con la app REAL instala cada uno con sus permisos, lo ejecuta en su iframe aislado, le dispara sus acciones, comprueba lo que hizo en la API y en disco (p. ej. `Bienvenida.md` y `Nueva nota.md`, historial vacío tras «Limpiar», medición guardada) y lo desinstala sin dejar nada. Otra prueba instala `apps` y `clipboard` **sin conceder permisos** y comprueba que todo se rechaza y que no escriben nada. Una tercera prueba, con ventanas WMTEST y el gestor real, comprueba que una app oculta en otro workspace se lista y que enfocarla cambia de workspace.
- 113 pruebas de la interfaz (Vitest) cubren la lógica de los cuatro módulos con una API falsa (incluida la seguridad del Markdown).

## Fase 9: Módulo de pareja 🟡 (preparada; falta el backend real)
- ✅ Módulo instalable `partner`: mensajes, avatar animado, botones rápidos, historial, ajustes, hueco en la barra.
- ✅ Núcleo: permiso `channels` y API `channels.*` con validación, límites y entrega solo a suscritos con permiso; backend de canales local y simulado (400 ms).
- ✅ Instalable por separado: la app sin `--manage` es un anfitrión de barra/panel/módulos.
- ✅ Pruebas: unitarias del hub y de la API (dos módulos intercambian mensajes con retardo medido), 13 de la lógica del módulo (dos instancias con documentos separados, retardo, avatar, persistencia, permisos) y una real con la app (`tests/phase9_partner.rs`: enviar, recibir la respuesta de la pareja virtual por el canal, guardar, desinstalar) más una sin permiso (todo rechazado).
- ⬜ **No cumplido**: «dos instancias se envían mensajes con delay razonable» **entre equipos reales** (necesita el servidor, ver B-10). Se cumple solo con el canal simulado.

## Fase 10: Instalación 🟡 (preparada; instalador y actualizaciones pendientes)
- ✅ **Arranque con Windows**: `infrastructure::windows::autostart` (valor en `HKCU\...\Run`), pestaña **General** con casilla; probado activando, consultando, reemplazando y desactivando en una clave de **pruebas** propia (la real no se tocó y sigue sin valor). **No está activado en tu equipo.**
- ✅ **Pestaña General**: estado (gestor y barra), activar gestor y barra (con confirmación y recordatorio de la tecla de emergencia), reiniciar sin gestor, arranque con Windows, carpetas, módulos incluidos y actualizaciones.
- ✅ **Compilación de producción** (`--features custom-protocol`, interfaz incrustada): compilada y **probada de extremo a extremo con los 7 casos reales de módulos** (fases 7, 8 y 9) sin servidor de desarrollo, incluida la comprobación de que el módulo aislado no puede hablar con Tauri.
- ✅ **Configuración del instalador** NSIS por usuario, en español, con los módulos como recurso (la compilación la valida).
- ⬜ **Instalador no generado** (`npm run tauri build` descarga NSIS: no lo ejecuté sin pedírtelo).
- ⬜ **Actualizaciones no conectadas**: código listo tras `--features updater` (compila), falta clave de firma y servidor de versiones.
- Guía completa en `docs/instalacion-y-actualizaciones.md`.

## Fase 11: Buscador de aplicaciones (2.0.0) 🟡 (código y pruebas hechos; falta tu prueba de uso real)
- ✅ **Dominio** (`domain/search.rs`): redirecciones y búsquedas web (solo `http(s)`, `{q}` solo en búsquedas y nunca en el servidor), codificación de lo escrito, coincidencia en tres niveles, uso reciente con envejecimiento (vida media de 30 días) y orden.
- ✅ **Aplicación** (`application/search.rs`): `SearchService` con identificadores opacos; abrir enfoca si está abierta, lanza si no, `ctrl+Enter` pide elevación; un identificador desconocido no hace nada.
- ✅ **Infraestructura**: `search.json` y `search-usage.json` (archivo dañado no rompe nada), `launch_elevated` (`runas`, cancelar el aviso no es error), `open_http`, iconos leídos del archivo real, cristal propio del buscador.
- ✅ **Atajo** `ctrl+shift+espacio` (`toggle_search`): también con el gestor en pausa; se añade solo al `config.toml` que ya tenías.
- ✅ **Ventana y vista**: ventana `search` precreada y dormida, mismo cristal y animaciones que el panel, respaldo de la tecla Windows; sección **Ajustes → Buscador**.
- ✅ Pruebas: 416 unitarias de Rust y 370 de la interfaz pasan (nuevas: dominio, aplicación con host falso, almacenes, migración de `config.toml`, `SearchView`, `SearchSettings`); 3 en vivo (`tests/live_search.rs`): el atajo real llega a la ventana también en pausa, el escáner real encuentra aplicaciones y el icono sale del archivo.
- ⬜ **Sin verificar con la app real abierta** (no se arrancó Tesela completa para no tocar tus ventanas): aspecto del cristal y las animaciones, que el cuadro de texto reciba el foco al abrir, que la tecla Windows siga siendo de Tesela con el buscador abierto, el aviso de control de cuentas con `ctrl+Enter`, y que abrir una app ya abierta cambie de workspace. Lista de comprobación en el resumen de la entrega; el criterio de §18.12 (desinstalar Flow Launcher) lo decides tú tras usarlo.
- ✅ **Mejoras tras la primera prueba** (D-107 y D-108): logo de cada página web en los resultados (su favicon, con el servicio de iconos de Google como respaldo), iconos de aplicaciones sin la flechita del acceso directo, icono de Wi-Fi reactivo a la señal en Controles, **apps de escritorio empaquetadas** (Teams, Claude, WhatsApp, Terminal…) con su logo, y **fondo de puntos interactivo en la barra de tareas**. Pruebas: 431 de Rust, 397 de la interfaz y 5 en vivo.
- ✅ **Calendario en «Hoy»**: un botón en la tarjeta «Esta semana» cambia los siete días por el mes completo (`MonthCalendar.svelte`, rejilla `monthGrid` en `stores/week.ts`), con flechas para cambiar de mes, el título para volver al mes de hoy y el día actual resaltado. Solo consulta fechas. Pruebas de la interfaz: 402.
- ✅ **Batería en la barra** (solo en equipos con batería: cargando con rayo, conectada, o con batería y tiempo restante; en rojo por debajo del 20 %) y **Configuración de Windows en el buscador** (Configuración y 35 páginas: Barra de tareas, Privacidad, Aplicaciones…, con el engranaje y sin elevar). D-109. Pruebas: 452 de Rust, 410 de la interfaz y 7 en vivo.
- ✅ **Clic en el borde de los botones**: al pulsar, el botón se encoge (`:active`) y un clic en su borde quedaba fuera del botón ya encogido, así que se veía la animación pero no se hacía la acción. `lib/ui/fluid.ts` (que ya vigila todos los botones) hace ahora el clic que faltaba si se suelta el ratón dentro de donde estaba el botón al pulsarlo. 416 pruebas de la interfaz.
- Fuera de lo pedido: las apps UWP puras de la Tienda (Calculadora, Ajustes…) no salen en los resultados (decisión de Erick); sí las empaquetadas que son programas de escritorio (`.exe`).
