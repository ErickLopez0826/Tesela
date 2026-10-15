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
