# DECISIONS

Decisiones tomadas durante el desarrollo autónomo. Las marcadas **(por defecto, pendiente de revisión)** son reversibles y esperan confirmación de Erick.

## Sección 11 del plan

1. **Sin espacio para otra columna** (por defecto, pendiente de revisión). Si añadir una columna rompería el ancho mínimo, la ventana nueva se abre como **flotante centrada**. No se rompe el mínimo ni se mueven las demás columnas.
2. **Monitor vertical** (por defecto, pendiente de revisión). El eje general del monitor es configurable (`columns` o `rows`); para monitores verticales el valor por defecto es `rows`. Ver la tensión con la instrucción de "apilado vertical" en BLOCKERS.md (B-1).
3. **alt+v** (por defecto, pendiente de revisión). Se mantiene. Alterna la orientación de apilado (vertical/horizontal) del grupo de la ventana enfocada. No cambia el modelo de columnas.

## Seguridad en pruebas y ejecución

- **D-1 Alcance de gestión explícito.** El gestor de ventanas NO arranca por defecto. Se activa con `--manage <alcance>`: `all` (todas las ventanas elegibles) o `prefix:TEXTO` (solo ventanas cuyo título empieza por TEXTO). Las pruebas usan `prefix:WMTEST` con ventanas creadas por `tools/spawn-test-windows.ps1`, así nunca se reorganizan ventanas del usuario.
- **D-2 Atajo de emergencia.** `Ctrl+Alt+Shift+F12` (fijo, no configurable en la Fase 1): pausa el gestor, devuelve cada ventana a su posición original y la vuelve a mostrar. Es independiente de `alt+shift+p` (pausa normal del plan, §8.7).
- **D-3 Restauración al salir.** Al cerrar la app (o con el atajo de emergencia) todas las ventanas ocultas se muestran y se restauran a su posición original. `--recover` hace lo mismo tras un cierre brusco, mostrando las ventanas que sigan ocultas por la app.
- **D-4 Energía en simulación.** Apagar/suspender/reiniciar/hibernar solo tienen implementación `dry-run` hasta que Erick confirme.

## Layout y modelo (secciones 4, 5, 7)

- **D-5 Proporciones.** Anchos y altos se guardan como proporciones que suman 1. Al agregar una columna, la nueva recibe 1/N y las demás se escalan en proporción (conservan su tamaño relativo). Al cerrar, las restantes se reescalan para sumar 1.
- **D-6 Mínimos.** `min_column_width` y `min_window_height` en píxeles. Al redimensionar, el reparto se hace con "llenado de agua": las columnas que tocan el mínimo se fijan en él y el resto absorbe el cambio en proporción.
- **D-7 Fullscreen ortogonal.** Pantalla completa y minimizada son banderas independientes del tipo de gestión (gestionada, excluida, flotante), porque el plan indica que una ventana en pantalla completa puede ser gestionada o excluida.
- **D-8 Ocultar por workspace.** Se intenta cloak de DWM; si el sistema lo rechaza para ventanas de otros procesos, se usa `SW_HIDE`. El resultado real se documenta en PROGRESS.md.

## Configuración (sección 8)

- **D-9 Formato.** TOML (`serde` + `toml`) para atajos y ajustes. Los atajos de referencia se tomaron de la sección 8 del plan.
