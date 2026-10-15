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

## Fase 2 (layout y monitores)

- **D-10 Redimensionar en el eje del apilado.** Si la ventana comparte grupo con otras y el eje pedido coincide con el del apilado, se redimensiona dentro del grupo; si no, se redimensiona la columna (eje principal). Con apilado horizontal en un eje de columnas, el ancho del grupo solo cambia cuando la ventana está sola.
- **D-11 Mover ventana.** Al mover a lo largo del apilado se reordena; al llegar al borde del apilado (o en la dirección del eje principal) la ventana pasa al grupo vecino (queda apilada con él). Si no hay grupo vecino y la ventana comparte grupo, sale a una columna nueva; si está sola, intenta pasar al monitor vecino.
- **D-12 Flotantes.** Siempre se centran al hacerse flotantes, con el 60 % × 70 % del área útil (`float_size`), porque el dominio no conoce el tamaño previo de la ventana.
- **D-13 Workspaces y monitores.** `bindings` liga un workspace a un monitor por posición en la lista (izquierda→derecha). Un workspace sin ligar se abre en el monitor donde estás. Cada monitor tiene exactamente un workspace visible. Los valores por defecto del código no ligan ninguno; la ligadura real saldrá de la configuración (Fase 3), basada en `~/.glzr/glazewm/config.yaml` (solo lectura): 1-4 → monitor 0, 5-7 → monitor 1, 8-9 libres.
- **D-14 Desconectar un monitor.** Sus workspaces pasan al principal (ocultos). Si el workspace estaba visible, sus ventanas se funden en el workspace visible del principal para que no queden ocultas. Al reconectar, los workspaces ligados vuelven a su monitor si estaban ocultos.
- **D-15 Ventanas de prueba.** El script de pruebas se ejecuta con una copia de `powershell.exe` llamada `Lively.exe` para que las reglas `ignore` de GlazeWM (proceso Lively + clase HwndWrapper) no hagan que ambos gestores se peleen por ellas. No se toca la configuración de GlazeWM.
- **D-16 Minimizar.** `alt+m` minimiza la ventana enfocada; si no hay enfocada, restaura la última minimizada. Una ventana minimizada sale del layout y vuelve a su columna al restaurarse.

## Fase 3 (atajos y configuración)

- **D-17 Archivo de configuración.** `%APPDATA%\window-manager\config.toml` (se crea con `src-tauri/default-config.toml` si no existe). Secciones `[layout]`, `[[workspace]]` y `[[keybinding]]` (acción, lista de combinaciones, modo opcional). Campos desconocidos se rechazan. Los valores por defecto de gaps (20 px, 60 px arriba) salen del `config.yaml` de GlazeWM (§16).
- **D-18 Pausa.** `alt+shift+p` y el atajo de emergencia hacen lo mismo: restauran todas las ventanas, liberan todos los atajos salvo el de pausa y dejan de gestionar. Al reanudar se vuelve a adoptar lo que haya. Así una pausa nunca deja ventanas ocultas.
- **D-19 Modo redimensionar.** Dentro del modo solo están registradas las teclas del modo (h/l/k/j, flechas, escape/enter), como en GlazeWM. Salir del modo restaura los atajos globales.
- **D-20 Recarga.** `alt+shift+r` relee el archivo; si es inválido (TOML, tecla/acción desconocida o misma combinación en dos acciones) no cambia nada y avisa con todos los errores (§13).
- **D-21 Interpretación de §8.8.** La regla "dos atajos no pueden estar asignados a la misma acción" contradice "un atajo puede tener varias combinaciones". Se implementó lo que pide §13: **una combinación no puede estar en dos acciones** dentro del mismo modo; una acción sí puede tener varias combinaciones.
- **D-22 Avisos.** Los avisos al usuario se escriben en stderr y se emiten como evento `wm-notice` de Tauri. La interfaz que los muestre llega con la barra (Fase 5).

## Fase 4 (exclusiones y pantalla completa)

- **D-23 Alcance de exclusiones.** Solo existe "siempre" (por ejecutable, con título como criterio adicional opcional). Los alcances por workspace/monitor/solo pantalla completa de §6.5 quedan fuera de la primera versión, como propone el propio plan.
- **D-24 Persistencia.** La lista vive en `%APPDATA%\window-manager\exclusions.json`, escrita por el panel (no se edita a mano). La primera vez se siembra con las reglas `ignore` del config.yaml de GlazeWM que se pueden expresar por ejecutable (juegos, explorer, zebar, PiP de navegadores, PowerToys Peek).
- **D-25 Reglas de ventana.** Las reglas "mover a workspace" y "flotante centrado" (§16) se guardan en `config.toml` (`[[window_rule]]`), migradas del config.yaml. Gestionarlas desde la interfaz queda pendiente (ver BLOCKERS B-5).
- **D-26 Detección de pantalla completa.** Una ventana está en pantalla completa si cubre todo su monitor y no tiene barra de título (juegos, F11, borderless). Se revisa cada 0,5 s para todas las ventanas seguidas, gestionadas o excluidas. Una gestionada en pantalla completa sale del layout y vuelve a su columna al salir (§7).
- **D-27 `alt+f`.** La ventana enfocada pasa a cubrir todo el monitor (incluida la zona de la barra) y oculta la barra; otra pulsación la devuelve a su columna.
- **D-28 Excluida con el foco.** Mientras una ventana excluida tenga el foco, los atajos que mueven, redimensionan, cierran, minimizan, flotan o envían a otro workspace no hacen nada (no actúan sobre otra ventana a sus espaldas). Los atajos de foco y de workspace siguen funcionando.
- **D-29 Señal de la barra.** La visibilidad de la barra por monitor sale como efecto `SetBar`; hasta la Fase 5 llega al frontend como evento Tauri `wm-bar-visibility` `[monitor, visible]`.
- **D-30 Orden de arranque.** Los atajos se registran antes de adoptar ventanas (se descubrió una carrera real: una pulsación justo tras adoptar se perdía).
