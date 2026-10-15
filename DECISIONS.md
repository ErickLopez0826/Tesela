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

## Fase 5 (barra y panel)

- **D-31 La barra se activa con `--bar`.** Acoplar una AppBar cambia el área útil de todos tus monitores, así que no arranca sola mientras desarrollo. El instalador/arranque con Windows (Fase 10) pasará `--manage all --bar`.
- **D-32 Barra como AppBar.** Una ventana de Tauri por monitor, sin marco ni foco (`WS_EX_NOACTIVATE`), acoplada arriba con `SHAppBarMessage`. Alto 40 px lógicos (se escala con el DPI del monitor). El área útil del monitor ya excluye la barra, así que el margen superior por defecto bajó de 60 px (que era para Zebar) a 20 px.
- **D-33 Atajo del panel (por defecto, pendiente de revisión).** §9.2 dice "con un clic o un atajo" sin fijar la tecla. Se añadió la acción `toggle_panel` con `alt+b`, que no choca con ningún atajo de §8 ni de GlazeWM. Es el único atajo por defecto fuera de §8.
- **D-34 Una página, tres ventanas.** `index.html?view=bar&monitor=<id>`, `?view=panel` y sin parámetro (ventana principal con ventanas/exclusiones).
- **D-35 Datos de la barra.** El núcleo publica un `StateSnapshot` (workspaces, enfocada, pausa, modo, visibilidad de barra) solo cuando cambia, como evento `wm-state` y consultable con `get_state`. CPU y RAM salen de `sysinfo` y la interfaz las lee cada 2 s. Los botones de workspace llaman `run_action("workspace_N")`.
- **D-36 Pantalla completa.** La barra se oculta con `SW_HIDE`/`SW_SHOWNOACTIVATE` pero conserva su franja reservada, para no reordenar todo cada vez que un juego entra o sale de pantalla completa.
- **D-37 Panel.** Ventana propia (380×560 lógicos) que aparece a la derecha, bajo la barra del monitor activo, y se cierra al perder el foco o con Escape.

## Fase 6 (módulos de sistema y de acción)

- **D-38 Servicios detrás de puertos.** Cada capacidad (medios, energía, conectividad, fondos, volumen, estadísticas) es un puerto en `application::system` con un adaptador de Windows en `infrastructure::windows::system`. La Fase 7 los envolverá como módulos con manifiesto y permisos sin cambiar los adaptadores.
- **D-39 Modo seguro (`[system] allow_changes = false`, por defecto).** Energía (apagar, suspender, reiniciar, hibernar) y conectividad (Wi-Fi, Bluetooth, compartir red) se **simulan**: el panel responde "Simulado: …" y el adaptador real ni se llama (probado con un adaptador falso). Para usarlos de verdad hay que poner `allow_changes = true` en `config.toml` y reiniciar la app. Además, la energía pide confirmación en dos pasos. Ningún comando de apagado fuerza el cierre de aplicaciones (sin `/f`).
- **D-40 Fondos.** La interfaz solo puede aplicar imágenes de la carpeta `%APPDATA%\window-manager\wallpapers` (nombre simple de archivo, sin rutas ni `..`). El cambio usa `IDesktopWallpaper::SetWallpaper`, la misma interfaz que Personalización de Windows, que aplica la transición del sistema. Elegir una imagen arbitraria del disco queda para cuando se decida añadir un selector de archivos nativo.
- **D-41 Medios.** Se usan las sesiones de transporte multimedia de Windows (SMTC): sirve para Spotify, navegadores y cualquier reproductor que aparezca en el control de volumen de Windows.
- **D-42 GPU.** Contadores PDH en inglés (`GPU Engine\Utilization Percentage`, independientes del idioma de Windows), agrupados por tipo de motor como el Administrador de tareas. Se lee en un hilo aparte.
- **D-43 Temperatura.** `sysinfo` no la expone en este equipo sin permisos de administrador; se muestra solo si está disponible (ver BLOCKERS B-7).
- **D-44 Hilos y COM.** El lector de estadísticas se crea de forma diferida porque `sysinfo` inicializa COM en modo multihilo y eso rompe la inicialización OLE del hilo principal de la ventana (lo descubrí al arrancar la app real). Las ventanas del panel se crean desde un hilo aparte, no desde el principal, para no bloquear el bucle de eventos.

## Fase 7 (módulos instalables)

- **D-45 Qué es un módulo.** Una carpeta con `module.json` + una página web (HTML/JS/CSS) que corre en un iframe aislado (`sandbox="allow-scripts"`, sin `allow-same-origin`). No hay código nativo de terceros: no existe forma de ejecutar un DLL ni de tocar Windows. Guía para autores en `modules/README.md`.
- **D-46 API del núcleo.** El módulo habla con el núcleo por `postMessage` (SDK servido por el núcleo en `__wm/sdk.js`); el anfitrión (Svelte) reenvía a `module_api` en Rust, que comprueba los permisos **concedidos**. El id del módulo lo fija el anfitrión según el iframe que envió el mensaje: un módulo no puede hacerse pasar por otro. Métodos: `core.version`, `windows.*`, `system.stats`, `power.run`, `notify.show`, `files.*`, `events.subscribe/unsubscribe`.
- **D-47 Permisos.** `windows`, `power`, `network`, `files`, `notifications`, `system`. Se muestran antes de instalar, el usuario marca cuáles concede (ninguno por defecto) y puede cambiarlos después. `power` no salta el modo seguro del núcleo (D-39). `network` se aplica con la política de seguridad (CSP) con la que el núcleo sirve el módulo: sin `network` concedido, `connect-src 'none'`; con él, solo los `network_hosts` declarados (sin comodines, solo `https`/`wss`).
- **D-48 Servir módulos.** Protocolo propio `wmmodule://` (en Windows `http://wmmodule.localhost/<id>/…`) servido por el núcleo con CSP `default-src 'none'; script-src 'self'` (sin scripts en línea ni `eval`), sin salir de la carpeta del módulo ni de sus enlaces simbólicos, y solo si el módulo está activo.
- **D-49 Instalación.** Desde una carpeta: vista previa (valida el manifiesto, límites 8 MB y 300 archivos, rechaza enlaces simbólicos y la carpeta reservada `__wm`) y después instalación atómica (se copia a una carpeta temporal y se renombra). Comprueba `min_core_version` y dependencias con versión mínima. Actualizar reemplaza archivos y conserva los datos del usuario; desinstalar borra archivos, datos propios y registro, y se niega si otro módulo depende de él.
- **D-50 Fallos.** Un método que falla (error del sistema o pánico) no tumba el núcleo; tras 3 fallos seguidos el módulo se desactiva y se cortan sus suscripciones. Los errores de uso (parámetros, permiso) no cuentan.
- **D-51 Eventos.** `workspace_changed` y `focus_changed`, solo a los módulos suscritos que siguen teniendo el permiso `windows` en el momento de emitir.
- **D-52 Sin firma ni catálogo (§10.5).** Se instalan carpetas que el usuario elige; lo que protege es el aislamiento y los permisos, no la confianza en el autor.
- **D-53 Prueba de extremo a extremo.** `--selftest-module <id>` (solo para pruebas): la ventana principal carga ese módulo, anota lo que pide a la API y escribe un informe en `WM_SELFTEST_OUT`. No hay estado real del escritorio en ese modo: se usa un estado de ejemplo.

## Fase 8 (módulos de utilidad)

- **D-54 Son módulos instalables de verdad.** `notes`, `apps`, `clipboard` y `speedtest` viven en `modules/` con su `module.json` y corren en el iframe aislado de la Fase 7; no tienen código en el núcleo más allá de la API que usan. No vienen preinstalados: se instalan desde la pestaña Módulos.
- **D-55 Permisos nuevos.** `launch` (abrir programas), `clipboard` y `speedtest`. Se añadieron porque ninguno de los permisos de la Fase 7 describe honestamente lo que hacen. `apps.list_open` y `apps.focus` cuelgan de `windows`.
- **D-56 Abrir aplicaciones.** `apps.launch` solo acepta rutas absolutas de `.exe` o `.lnk`, sin parámetros, sin `..` y sin caracteres de control: abre una aplicación, no ejecuta comandos. Se abre con `ShellExecute`.
- **D-57 Apps abiertas.** La lista incluye las ventanas que la propia app oculta por estar en otro workspace. Enfocar una cambia de workspace, la restaura si estaba minimizada y le da el foco (mensaje `FocusWindow` del gestor); sin gestor activo se enfoca directamente.
- **D-58 Fijadas "en configuración" (§9.3).** Se guardan en `fijadas.json` dentro de la carpeta de datos del módulo (`module-data\apps\`), no en `config.toml`: el módulo no tiene acceso a la configuración del núcleo por diseño.
- **D-59 Portapapeles y privacidad.** El núcleo no entrega el contenido si quien copió pidió que no entrara en historiales (`ExcludeClipboardContentFromMonitorProcessing` o `CanIncludeInClipboardHistory = 0`, lo que usan los gestores de contraseñas). El módulo ignora textos de más de 5000 caracteres y guarda 50 entradas. La barra y el panel ejecutan el mismo módulo: releen el archivo antes de escribir para no pisarse. **El historial se guarda en texto plano** en `module-data\clipboard\`.
- **D-60 Prueba de velocidad.** Con `speedtest.exe` en el PATH se usa el CLI de Ookla (`--format=json`); si no, un adaptador simulado devuelve datos de ejemplo marcados como tales. La medición corre en un hilo; el módulo la inicia y consulta el estado cada segundo (el SDK corta las llamadas a los 15 s).
- **D-61 Pruebas de extremo a extremo.** `--selftest-module` ahora dispara también las acciones que el módulo declara. En ese modo el portapapeles es un texto fijo para no leer ni guardar nunca el real.
