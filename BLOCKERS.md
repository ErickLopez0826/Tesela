# BLOCKERS

Lo que no pude resolver o requiere una decisión/acción de Erick.

## B-0 El mensaje de instrucciones llegó cortado
El último punto del entregable ("DECISIONS.md con todas las decisiones tomadas, incluidas las tres de la sección 11 con su…") se cortó. Asumí que pide la lista de decisiones con su estado y justificación. Además PROGRESS.md no existía al empezar: la Fase 0 era el único trabajo hecho, así que la "verificación previa" de las fases 1 y 2 dio como resultado que no estaban implementadas.

## B-1 Tensión en la decisión 2 (monitor vertical)
Se pide a la vez "apilado dentro de una columna en vertical" y "eje general por defecto = filas" para monitores verticales. Con eje en filas, cada grupo es una banda horizontal y apilar en vertical dentro de una banda no tiene sentido. Propuesta implementada: el apilado se expresa en términos absolutos (vertical/horizontal) y el valor por defecto de un grupo nuevo es el eje cruzado al eje principal (en eje `rows`, los grupos son bandas y sus ventanas van lado a lado). `alt+v` permite invertirlo. Si prefieres otra cosa, es un cambio localizado en `domain::layout`.

## B-2 Cloak de DWM no disponible
El plan (§12 Fase 1, §17) pide ocultar por workspace con cloak de DWM. `DwmSetWindowAttribute(DWMWA_CLOAK)` sobre ventanas de otro proceso devuelve `0x80070005 Acceso denegado` con un proceso normal sin elevar (`src-tauri/tests/cloak_probe.rs`). GlazeWM lo consigue con `hide_method: cloak`, probablemente por cómo está firmado/instalado (posible UIAccess). Alternativa implementada: `SW_HIDE`, con archivo de recuperación (`--recover`). Posible mejora: probar elevado (la app "puede correr como administrador", §2) — no lo hice porque requiere un aviso UAC. Propuesta: que Erick pruebe `cargo test --test cloak_probe -- --ignored --nocapture` desde una terminal de administrador; si da `Ok`, basta cambiar el orden de preferencia (ya soportado).

## B-3 Conflicto de atajos con GlazeWM
GlazeWM sigue corriendo y ya registra `alt+h`, `alt+1`, etc. Dos programas no pueden registrar el mismo atajo global. Las pruebas de la Fase 3 usan un modificador distinto (configurable). Para usar la app en el día a día habrá que detener GlazeWM a mano (yo no lo detengo).

## B-4 Redacción de §8.8
"Dos atajos no pueden estar asignados a la misma acción" choca con "un atajo puede tener varias combinaciones, como ahora" (alt+h y alt+left hacen lo mismo). Implementé la lectura de §13: una *combinación* no puede ir a dos acciones. Conviene corregir la redacción en el plan.

## B-5 Pendientes de la Fase 4
- Las reglas de ventana (`[[window_rule]]`) se editan en `config.toml`; el plan (§16) pide también gestionarlas desde la interfaz. No hay panel para eso todavía.
- Reglas de GlazeWM no migradas por depender de la clase de ventana (no está en el plan): Excel/Word/PowerPoint distintos de su ventana principal, Lively (clase HwndWrapper) y PowerToys PowerAccent. También las que solo usan título (Isaac, Mewgenics...): la exclusión es por ejecutable (§6.3). Se pueden añadir desde el panel.
- Bordes de ventana enfocada/no enfocada con colores (§16, "conceptos que se conservan") no aparecen en ninguna fase de §12; no se implementaron. Propuesta: añadirlos en la Fase 5 con DWMWA_BORDER_COLOR.
- No pude comprobar visualmente el panel de exclusiones dentro de la ventana de Tauri (solo pruebas de componente con Vitest y el comando Rust).

## B-6 Pendientes de la Fase 5
- §9.1 pide "contenido configurable por zonas (izquierda, centro, derecha)". Hoy el contenido de la barra es fijo (workspaces, ventana enfocada, CPU, RAM, reloj, botón del panel). Hacerlo configurable depende del sistema de módulos (Fase 7); lo retomo ahí.
- Volumen, temperatura y medios de la barra (§9.1) llegan con los módulos de la Fase 6.
- Bordes de ventana (§16) siguen sin implementar (ver B-5).
- Verifiqué visualmente la barra (captura real de pantalla: CPU, RAM, reloj y botón del panel), pero no el panel desplegable ni el comportamiento con varios monitores a nivel visual; el panel y la barra están cubiertos por pruebas de componente y la reserva de espacio por una prueba real con tus dos monitores.

## B-7 Pendientes de la Fase 6
- **Acciones reales sin probar**: por tus reglas de seguridad nunca ejecuté apagar/suspender/reiniciar/hibernar ni cambié Wi-Fi/Bluetooth/compartir red, ni envié órdenes a tu reproductor, ni cambié tu fondo o tu volumen. Lo que está verificado con el sistema real es **leer** (CPU, GPU, RAM, discos, red, apps que más consumen, volumen, lo que suena en Spotify, fondo actual, estado de Wi-Fi/Bluetooth/compartir red) y que en modo seguro el adaptador real nunca se llama. Los caminos "de escritura" están cubiertos por pruebas con adaptadores falsos y por revisión de código. **Para que los pruebes tú**: pon `allow_changes = true` en `%APPDATA%\window-manager\config.toml`, reinicia la app y usa los botones del panel (empieza por Wi-Fi/Bluetooth y fondos; la energía al final).
- **Fondo y Wallpaper Engine**: tu fondo actual lo gestiona Wallpaper Engine (`WallpaperEngineOverride_….jpg`). Cambiarlo con `IDesktopWallpaper` puede chocar con esa app. Conviene probarlo con cuidado.
- **Temperatura**: `sysinfo` no devuelve sensores en este equipo sin elevar. Opciones: ejecutar como administrador, o integrar LibreHardwareMonitor (instalar software: requiere tu aprobación).
- **Transición del fondo**: confío en la transición nativa de Windows al usar `IDesktopWallpaper`; no pude comprobar visualmente que haga fundido sin cambiar tu fondo real.
- **Panel dentro de la ventana real**: verifiqué visualmente el diseño del panel con datos simulados (captura con Edge sin cabeza) y que la ventana del panel se crea y se muestra con la app real, pero con GlazeWM todavía corriendo este reposiciona las ventanas de la app (la ventana principal y el panel). Es un efecto de mantener GlazeWM como respaldo, no de la app.
- Capturas de pantalla: en una comprobación visual capturé por error contenido de tus monitores (Steam, Spotify, Discord). No guardé ni usé nada de eso; ya no hago capturas de pantalla reales.

## B-8 Pendientes de la Fase 7
- **Aislamiento frente a Tauri**: dentro del iframe sí existe `window.__TAURI_INTERNALS__`, pero una llamada directa a un comando (`modules_list`) **no obtiene respuesta** (probado en la app real, esperando 8 s). La sonda de `tests/phase7_modules.rs` falla si algún día Tauri empezara a contestar. Es una garantía empírica de esta versión de Tauri/WebView2, no una declarada en su documentación; conviene volver a ejecutar esa prueba al actualizar Tauri.
- **`network`** se aplica solo con la CSP del módulo; no pude probarlo de extremo a extremo (haría falta un servidor y un módulo de red). Lo necesitará el módulo de pareja (Fase 9).
- El slot de la **barra** (`ui.bar`) y los botones de **acciones** del panel están implementados y probados con Vitest, pero no los vi renderizados en la app real (la prueba real usa la ventana principal).
- **Atajos sugeridos** por un módulo: se muestran al instalar pero no se agregan solos a `config.toml`.
- **Instalar** pide una carpeta escrita a mano (no hay selector de carpetas nativo ni formato empaquetado `.zip`); añadirlo es trivial pero requiere decidir formato y una dependencia nueva.
- **Eventos de red** (`cambio de red`, §10.4) y `enviar mensaje a un canal` no existen todavía: el segundo es del módulo de pareja (Fase 9).
- Los módulos **no se instalan automáticamente en la barra**: `ui.bar` solo aparece si el módulo lo declara.

## B-9 Pendientes de la Fase 8
- **CLI de Ookla no instalado** (no instalo software sin tu aprobación). Hasta entonces la prueba de velocidad devuelve datos de EJEMPLO marcados como simulados. Para medir de verdad ejecuta tú, en una terminal:
  `winget install Ookla.Speedtest.CLI`
  (la primera ejecución del CLI pide aceptar la licencia; la app ya pasa `--accept-license --accept-gdpr`). Después reinicia la app: la detecta sola en el PATH. El análisis de su salida está probado con una salida de ejemplo del formato documentado, pero **no con una medición real**.
- **Portapapeles real sin probar de extremo a extremo**: por privacidad la prueba usa un texto fijo. La lectura real del portapapeles está verificada (no falla) pero no escribí en él; hay una prueba manual con restauración (`cargo test -- --ignored escribir_y_leer_con_restauracion`).
- **Abrir/fijar apps con `apps.launch` real**: probado con adaptadores falsos (validación y llamadas) y la lista y el enfoque con ventanas reales; no abrí ningún programa de verdad.
- **Historial del portapapeles en texto plano** (D-59): si copias contraseñas desde algo que no las marca como privadas, quedarían en `module-data\clipboard\historial.json`. Propuesta: opción de cifrado o desactivarlo por aplicación de origen.
- **Notas**: el Markdown es básico (títulos, negrita, cursiva, código, listas, bloques); no hay tablas, enlaces ni imágenes (a propósito, para que renderizar una nota no pueda ejecutar nada).
- Los módulos se instalan indicando una carpeta escrita; como no vienen preinstalados, la primera vez hay que instalarlos a mano desde `modules\`.

## B-10 Fase 9: lo que falta para el módulo de pareja real
Detalle y plan en `docs/modulo-pareja.md`. En resumen, necesito de ti:
- **Aprobación y datos de un proyecto de Supabase** (no creé proyectos, credenciales, secretos ni funciones: reglas de seguridad). Hace falta la URL y la clave `anon`; las claves nunca van en el código, irán en `config.toml`.
- **`RemoteChannelBackend`** (cliente de Realtime sobre WebSocket): sin escribir ni probar porque no se puede verificar sin servidor; la interfaz (`ChannelBackend`) ya existe y está probada con el backend local. Requerirá una dependencia de WebSocket (p. ej. `tungstenite`), que no está en §14.
- **Autenticación y política por pareja** (propuesta en el documento).
- **Diseño del avatar y su comportamiento** (§10.6): el actual es un marcador de posición.
- **Criterio de §12 Fase 9** («dos instancias se envían mensajes con delay razonable»): cumplido **solo con el canal simulado** dentro de una misma app (prueba unitaria con dos suscriptores: entrega entre 20 y 500 ms, y prueba real del módulo con la pareja virtual: ida y vuelta en ~1,6 s). Con dos equipos reales **no está cumplido**.
- Sin cola de mensajes sin conexión ni confirmación de entrega: un mensaje a un canal sin suscriptores se pierde (como en una difusión).

## B-11 Fase 10: lo que falta para «se instala con un solo archivo» y «se actualiza»
Guía paso a paso en `docs/instalacion-y-actualizaciones.md`.
- **El instalador no está generado.** `npm run tauri build` descarga NSIS a la caché de Tauri la primera vez y no lo ejecuté para no instalar herramientas en tu equipo sin pedírtelo. Lo que sí hice: configurar el instalador (la compilación valida la configuración), compilar el programa de producción y probarlo. **Ejecuta tú `npm run tauri build`** cuando quieras el `.exe`.
- **Firma de código**: sin certificado, SmartScreen avisará de «editor desconocido». No firmé nada.
- **Actualizaciones**: faltan (1) la clave de firma del actualizador (`tauri signer generate`; secreta), (2) un lugar donde publicar `latest.json` y el instalador (p. ej. GitHub Releases) y (3) compilar con `--features updater`. Hasta entonces la pestaña General lo dice. Criterio «se actualiza» de §12: **no cumplido**.
- **Arranque con Windows** verificado contra una clave de pruebas del registro, **no** contra el arranque real ni reiniciando sesión. Además activarlo mientras GlazeWM también arranca solo provoca peleas por las ventanas y los atajos.
- **Icono definitivo**: el icono es un cuadrado azul provisional de la Fase 0. Necesito tu diseño.
- **Primer arranque**: la app instalada se abre sin gestor ni barra (modo seguro); hay que activarlos en la pestaña General o con el arranque automático. No hay asistente de primer uso.
- **Versión**: sigue siendo `0.1.0`; falta decidir tu política de versiones antes de la primera publicación.
- **Pruebas en otro equipo**: todo lo probado se probó en este equipo (Windows 11, dos monitores). No hay pruebas con otro DPI, otros monitores ni sin WebView2 preinstalado.
