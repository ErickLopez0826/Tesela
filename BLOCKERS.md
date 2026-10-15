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
