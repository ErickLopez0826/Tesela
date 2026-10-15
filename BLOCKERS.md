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
