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
