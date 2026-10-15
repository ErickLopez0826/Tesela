# PROGRESS

Estado por fase de la sección 12. Leyenda: ✅ cumplido · 🟡 parcial · ⬜ pendiente.

Verificación previa (estado inicial): PROGRESS.md no existía; solo estaba hecha la Fase 0. Las fases 1 y 2 no estaban implementadas y se hicieron desde cero.

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
