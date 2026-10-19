# Window Manager App: Documento de planificación

Versión: 0.3 (planificación; el código avanza por fases, ver PROGRESS.md; la sección 18 describe el buscador, implementado en la versión 2.0.0)
Autor: Erick
Estado: Pendiente de cerrar tres decisiones marcadas en la sección 11

---

## 1. Objetivo

Crear una sola aplicación de escritorio para Windows que reemplace por completo a **GlazeWM** y **Zebar**, con:

- Gestión de ventanas por **columnas** con reparto equitativo del espacio.
- Workspaces por monitor, con atajos de teclado idénticos a los actuales.
- Una barra superior por monitor y un panel desplegable con controles del sistema.
- Exclusión de aplicaciones que no deben ser gestionadas.
- Un sistema de **módulos instalables**, para que otras personas (por ejemplo, la pareja de Erick) puedan instalar solo la funcionalidad que necesiten.

GlazeWM se mantiene instalado como respaldo durante todo el desarrollo.

---

## 2. Decisiones ya tomadas

| Tema | Decisión |
|---|---|
| Relación con GlazeWM | La app **reemplaza** GlazeWM. GlazeWM queda instalado como respaldo. |
| Relación con Zebar | La app **reemplaza** Zebar. La barra es nativa de la app. |
| Quién gestiona el layout | La app. No se usa el layout de GlazeWM. |
| Plataforma | Windows 11 (el equipo de Erick). |
| Stack | **Tauri 2**: núcleo en Rust, interfaz web. |
| Frontend | Svelte recomendado (sin preferencia fija de Erick). |
| Permisos | La app puede correr como administrador. |
| Layout | Reparto equitativo por **columnas**. |
| Nueva ventana | Se agrega como columna a la **derecha**. |
| Redimensionar | Todas las demás columnas se reajustan **en proporción**. |
| Mínimo | Las columnas se achican hasta un **ancho mínimo**. |
| Vertical dentro de columna | Una columna puede contener **varias ventanas apiladas**, que se reparten el alto. |
| Atajos | Los mismos que el `config.yaml` actual, sin cambios. Configurables desde un archivo. |
| Exclusiones | Se seleccionan desde la interfaz, no en un archivo. Solo para el equipo de Erick. |
| Pantalla completa | Cualquier ventana en pantalla completa oculta la barra en ese monitor. |
| Ventana excluida | Se abre donde Windows la coloca por defecto. |
| Notas | Markdown o texto plano, guardadas en una carpeta local. |
| Prueba de velocidad | CLI oficial de Ookla. |
| Apps en el panel | Dos listas: **abiertas** y **fijadas**, intercambiables. |
| Módulo de pareja | Instalable por separado. Mensajes en tiempo real con delay razonable. Avatar animado. Último en desarrollarse. |
| ai-bridge.py | Fuera de alcance por ahora. Muestra tokens de Claude y Gemini. Se retoma después. |
| Buscador de aplicaciones | Tesela tendrá su **propio buscador** para poder eliminar Flow Launcher. Se abre con **ctrl+shift+espacio**. Detalle en la sección 18. **Implementado en la versión 2.0.0** (D-106 en DECISIONS.md). |

---

## 3. Arquitectura general

La aplicación tiene cuatro piezas:

1. **Núcleo**: el único componente que habla directamente con Windows. Gestiona ventanas, workspaces, layout, atajos, exclusiones, configuración y el cargador de módulos.
2. **Interfaz del sistema**: barra superior por monitor y panel desplegable. Es la capa visual que reemplaza a Zebar.
3. **Cargador de módulos**: descubre, valida permisos, carga y descarga módulos instalables.
4. **Módulos**: funcionalidades independientes. Pueden ser de sistema (CPU, RAM, red), de acción (energía, fondos, Wi-Fi, Bluetooth), de utilidad (notas, apps, portapapeles, velocidad) o de terceros (mensajes de pareja).

### Principio de diseño

El núcleo no conoce los módulos. Los módulos no tocan Windows directamente: piden lo que necesitan a través de una **API definida por el núcleo**, y el núcleo decide si se lo permite según sus permisos.

### Organización por capas (arquitectura onion)

- **Dominio**: reglas puras, sin Windows ni interfaz. Layout, workspaces, estados de ventana, reglas de exclusión, parseo de atajos. Debe poder probarse sin abrir ninguna ventana real.
- **Aplicación**: casos de uso. Ejemplos: "mover ventana a workspace", "aplicar layout", "alternar pantalla completa", "cambiar fondo con transición".
- **Infraestructura**: adaptadores a Windows (eventos, `SetWindowPos`, DWM para cloak, API de energía, Wi-Fi, Bluetooth), a Ookla CLI, al sistema de archivos y a Supabase cuando llegue el módulo de pareja.
- **Interfaz**: componentes de barra, panel y módulos. Un store por dominio.

---

## 4. Modelo de datos

### Monitores
- Cada monitor tiene su orientación real (horizontal o vertical), su resolución y su área útil (descontando la barra).
- Cada monitor tiene un conjunto de workspaces asignados.

### Workspaces
- Cada workspace pertenece a un monitor.
- Tiene una lista ordenada de **columnas**.
- Puede estar visible u oculto. Un workspace oculto conserva su estructura completa.
- Tiene una ventana enfocada.

### Columnas
- Cada columna tiene una lista ordenada de **ventanas apiladas**.
- Tiene un ancho (proporción del espacio disponible).

### Ventanas
- Cada ventana tiene un **estado de gestión** (ver sección 7) y un **ejecutable** asociado.
- Dentro de una columna, cada ventana tiene un alto (proporción del alto de la columna).

---

## 5. Motor de layout

### 5.1 Reparto de ancho

- Con N columnas, el ancho disponible se reparte entre ellas.
- Al agregar una columna, todas se reajustan para repartirse el espacio.
- Al redimensionar una columna, las demás se reajustan **en proporción a su tamaño actual**. La suma siempre es igual al ancho disponible.
- Ninguna columna puede bajar de un **ancho mínimo** configurable.

### 5.2 Reparto de alto dentro de una columna

- Con M ventanas apiladas en una columna, el alto de la columna se reparte entre ellas.
- Al redimensionar una ventana en vertical, las demás de su columna se reajustan en proporción.
- Cada ventana tiene un **alto mínimo** configurable.

### 5.3 Inserción de ventanas

- Una ventana nueva se agrega como **columna a la derecha** de la última columna del workspace.
- Excepción: si la ventana nueva es hija de otra (por ejemplo, un diálogo), sigue la regla de exclusión o de flotante, no la de layout.

### 5.4 Cierre y movimiento

- Al cerrar una ventana, su espacio se reparte de nuevo entre las demás de la columna. Si la columna queda vacía, desaparece y las demás se reparten el ancho.
- Al mover una ventana a otra columna, la columna origen se reajusta.
- Al mover una ventana fuera de un workspace, sale de su estructura y entra en la del destino.

### 5.5 Monitores verticales

- Cada monitor calcula su layout por separado.
- **Pendiente de confirmar** (ver sección 11): en un monitor vertical, ¿el eje principal sigue siendo por columnas, o por filas?

### 5.6 Caso límite: no cabe otra columna

- Cuando el ancho disponible ya no permite más columnas respetando el mínimo, la ventana nueva **no** se fuerza dentro.
- **Pendiente de confirmar** (ver sección 11): qué pasa en ese caso.

---

## 6. Exclusiones

### 6.1 Concepto

Una ventana excluida es una ventana que la app **no gestiona**. Se comporta como si GlazeWM no existiera para ella.

### 6.2 Qué hace la app con una ventana excluida

- **No** la mueve, redimensiona ni posiciona.
- **No** la cuenta para el reparto de columnas.
- **No** le aplica bordes, gaps ni atajos de posición.
- **Sí** sigue escuchando sus eventos para saber cuándo se cierra y limpiar el estado.
- **Sí** respeta sus cambios a pantalla completa (ver 6.4).
- Se abre donde Windows la coloca por defecto.

### 6.3 Cómo se seleccionan

Desde un **panel de exclusiones** en la interfaz, con dos formas:

1. **Desde ventanas abiertas**: lista de ventanas activas, con un clic para marcar "no gestionar".
2. **Desde búsqueda de ejecutables**: escribir el nombre del proceso o elegir un `.exe` del disco.

La exclusión se guarda por **nombre de ejecutable**, no por título.

### 6.4 Pantalla completa

- Cuando **cualquier** ventana entra en pantalla completa, la barra se oculta en ese monitor, sin importar si es juego o aplicación.
- Al salir de pantalla completa, la barra reaparece.
- Esto aplica a ventanas gestionadas y excluidas.

### 6.5 Alcance de una exclusión

Cada exclusión puede tener un alcance:

- Siempre.
- Solo en ciertos workspaces o monitores.
- Solo en pantalla completa.

**Pendiente de confirmar**: el alcance por workspace o monitor no fue mencionado explícitamente por Erick. Se deja como opción posterior si no se necesita en la primera versión.

### 6.6 Alcance del uso

- Las exclusiones son **solo del equipo de Erick**.
- No se sincronizan con el módulo de pareja.

### 6.7 Título como criterio secundario

- El título de la ventana **no** es criterio principal.
- Se reserva para casos donde un mismo proceso tiene ventanas que sí deben gestionarse y otras que no (por ejemplo, Chrome con Picture-in-Picture).
- Se configura como un criterio adicional, no como regla independiente.

---

## 7. Estados de una ventana

Una ventana siempre está en uno de estos estados:

| Estado | Entra en layout | Barra | Notas |
|---|---|---|---|
| **Gestionada** | Sí | Visible | Estado normal. |
| **Excluida** | No | Visible | Se abre donde Windows la coloca. |
| **Flotante** | No | Visible | Centrada al abrirse, o según la regla. |
| **Pantalla completa** | No | Oculta en ese monitor | Puede ser gestionada o excluida. |
| **Minimizada** | No | Visible | Vuelve a su posición al restaurar. |

Transiciones relevantes:

- Gestionada → Flotante: con el atajo de flotante (alt+shift+space).
- Flotante → Gestionada: al volver a tiling, regresa a su posición en la estructura, o se agrega al final si ya no existe.
- Cualquiera → Pantalla completa: la barra se oculta. Al salir, reaparece.
- Gestionada → Excluida: al marcarla en el panel de exclusiones. Sale del layout y se queda donde está.

---

## 8. Atajos de teclado

Los atajos actuales del `config.yaml` se conservan **sin cambios**. Se definen en un archivo de configuración editable, no en código.

### 8.1 Foco entre ventanas
- alt+h / alt+izquierda: foco izquierda
- alt+l / alt+derecha: foco derecha
- alt+k / alt+arriba: foco arriba
- alt+j / alt+abajo: foco abajo

### 8.2 Mover ventana
- alt+shift+h / alt+shift+izquierda: mover izquierda
- alt+shift+l / alt+shift+derecha: mover derecha
- alt+shift+k / alt+shift+arriba: mover arriba
- alt+shift+j / alt+shift+abajo: mover abajo

### 8.3 Redimensionar
- alt+u: achicar ancho (−2%)
- alt+p: agrandar ancho (+2%)
- alt+o: agrandar alto (+2%)
- alt+i: achicar alto (−2%)

### 8.4 Modo redimensionar
- alt+r: activar modo redimensionar.
- Dentro del modo: h / izquierda, l / derecha, k / arriba, j / abajo, para redimensionar.
- Escape o Enter: salir del modo.

### 8.5 Workspaces
- alt+1 a alt+9: cambiar al workspace.
- alt+shift+1 a alt+shift+9: mover la ventana enfocada al workspace y seguirla.
- alt+s: siguiente workspace activo.
- alt+a: anterior workspace activo.
- alt+d: workspace reciente.
- alt+shift+a: mover workspace a la izquierda (entre monitores).
- alt+shift+f: mover workspace a la derecha.
- alt+shift+d: mover workspace arriba.
- alt+shift+s: mover workspace abajo.

### 8.6 Ventana
- alt+v: alternar dirección de división. **Pendiente de confirmar** si sigue existiendo con el modelo de columnas (ver sección 11).
- alt+t: alternar tiling / flotante.
- alt+f: alternar pantalla completa.
- alt+m: minimizar / restaurar.
- alt+shift+space: alternar flotante centrado.
- alt+shift+q: cerrar ventana.
- alt+space: ciclar foco entre ventanas.

### 8.7 Sistema
- alt+enter: abrir terminal.
- alt+shift+p: pausar el gestor (suspender manejo de ventanas).
- alt+shift+r: recargar configuración.
- alt+shift+w: redibujar.
- alt+shift+e: salir de la app.

### 8.8 Regla general
- Todos los atajos son configurables desde el archivo de configuración.
- Dos atajos no pueden estar asignados a la misma acción.
- Un atajo puede tener varias combinaciones, como ahora.

---

## 9. Barra y panel

### 9.1 Barra superior

- Una barra por monitor, acoplada al borde superior como AppBar de Windows (reserva su espacio).
- Contenido configurable por zonas (izquierda, centro, derecha).
- Módulos por defecto: workspaces, ventana enfocada, reloj, CPU, RAM, volumen, temperatura, medios.
- Se oculta cuando hay una ventana en pantalla completa en ese monitor (ver 6.4).

### 9.2 Panel desplegable

- Se abre desde la barra, con un clic o un atajo.
- Secciones (cada una puede ser un módulo):
  - Controles de medios.
  - Estadísticas del sistema (CPU, GPU, RAM, disco, red, tiempo encendido, apps que más consumen).
  - Prueba de velocidad (CLI de Ookla).
  - Energía: apagar, suspender, reiniciar, hibernar.
  - Conectividad: Wi-Fi, Bluetooth, compartir red.
  - Fondos de pantalla: cambio directo con transición.
  - Apps: listas de **abiertas** y **fijadas**, intercambiables.
  - Notas: markdown o texto plano, para anotar rápido.

### 9.3 Apps abiertas y fijadas

- Dos listas, con un selector para cambiar entre ellas.
- Las fijadas se guardan en configuración.
- Las abiertas se toman del sistema en tiempo real.

---

## 10. Sistema de módulos instalables

### 10.1 Principio

Cada funcionalidad opcional es un módulo. La app base incluye el núcleo y algunos módulos de sistema. Otros módulos pueden instalarse por separado.

### 10.2 Manifiesto

Cada módulo declara:

- Nombre, versión, autor, descripción.
- Versión mínima del núcleo que necesita.
- **Permisos** que solicita (por ejemplo: red, archivos, notificaciones, ventanas, energía).
- Dependencias con otros módulos, si las hay.
- Puntos de entrada: qué componentes de interfaz aporta, qué acciones registra, qué atajos sugiere.

### 10.3 Permisos

- El núcleo solo entrega a cada módulo los permisos que declaró en su manifiesto y que el usuario aceptó.
- Un módulo de mensajes no necesita acceso a Wi-Fi ni a energía.
- Los permisos se muestran al usuario antes de instalar.

### 10.4 Comunicación

- Los módulos llaman a una **API del núcleo**, no a Windows directamente.
- La API expone operaciones de alto nivel (por ejemplo: "obtener workspace activo", "ejecutar comando de energía", "enviar mensaje a un canal").
- Los módulos pueden suscribirse a eventos del núcleo (cambio de workspace, ventana enfocada, cambio de red).

### 10.5 Confianza

- Los módulos de Erick y su pareja son de confianza bajo control de ambos.
- Si en el futuro se aceptan módulos de terceros, se necesita un mecanismo de firma o un catálogo controlado. Esto no entra en la primera versión.

### 10.6 Módulo de pareja (último en desarrollarse)

- Se instala por separado, sin la app completa.
- Incluye un avatar animado que se muestra al abrirlo.
- Permite enviarse cosas en tiempo real, con delay razonable.
- Requiere un backend de sincronización. Opción recomendada: **Supabase** (canales en tiempo real).
- Se desarrolla después de que el sistema de módulos esté funcionando.
- El diseño del avatar y su comportamiento aún no están definidos.

---

## 11. Decisiones pendientes

Estas tres preguntas quedaron sin respuesta clara y deben cerrarse antes o durante la fase 1:

1. **Caso límite**: cuando ya no cabe otra columna respetando el mínimo, ¿la ventana nueva se abre como flotante, se manda al siguiente workspace, o se rompe el mínimo con desplazamiento horizontal?
2. **Monitor vertical**: ¿el reparto principal sigue siendo por columnas (que se ven como filas) o por filas, que es lo natural para ese monitor?
3. **alt+v**: ¿se mantiene para alternar entre columnas y filas dentro de un workspace, o se elimina?

Otras pendientes de menor prioridad:

- Alcance de exclusiones por workspace o monitor (puede esperar).
- Diseño del avatar del módulo de pareja.
- Qué hacer con `ai-bridge.py` (fuera de alcance por ahora).

---

## 12. Fases de desarrollo

Cada fase tiene un criterio de terminado. No se pasa a la siguiente sin cumplirlo.

### Fase 0: Base
- Proyecto Tauri creado, con estructura de capas.
- Ventana de prueba que enumera ventanas del sistema.
- **Terminado cuando**: la app lista las ventanas visibles con su ejecutable y su título.

### Fase 1: Núcleo de ventanas
- Escuchar eventos de ventanas (abrir, cerrar, enfocar, minimizar, mover).
- Mover y redimensionar ventanas con `SetWindowPos`.
- Ocultar y mostrar ventanas por workspace (técnica cloak de DWM).
- **Terminado cuando**: se puede abrir, cerrar y cambiar entre varias ventanas sin que se pierdan ni se queden fuera de pantalla.

### Fase 2: Motor de layout
- Modelo de columnas y ventanas apiladas.
- Reglas de inserción, redimensionamiento proporcional, mínimos y cierre.
- Pruebas unitarias del dominio, sin abrir ventanas reales.
- **Terminado cuando**: todas las reglas de la sección 5 pasan sus pruebas (ver sección 13).

### Fase 3: Atajos y configuración
- Cargar atajos desde archivo editable.
- Migrar todos los atajos de la sección 8.
- Recarga en caliente con alt+shift+r.
- **Terminado cuando**: cada atajo de la sección 8 ejecuta su acción correcta.

### Fase 4: Exclusiones y pantalla completa
- Panel para seleccionar ventanas o ejecutables a excluir.
- Comportamiento de ventana excluida (sección 6.2).
- Ocultar barra en pantalla completa (sección 6.4).
- **Terminado cuando**: un juego y una aplicación excluida se abren y funcionan como si GlazeWM no existiera.

### Fase 5: Barra y panel
- Barra por monitor con AppBar.
- Panel desplegable con secciones básicas (reloj, CPU, RAM, workspaces).
- **Terminado cuando**: la barra reserva su espacio y no se encima con las ventanas.

### Fase 6: Módulos de sistema y acción
- Medios, estadísticas, red, energía, Wi-Fi, Bluetooth, fondos con transición.
- **Terminado cuando**: cada acción del panel funciona desde la interfaz.

### Fase 7: Sistema de módulos instalables
- Cargador, manifiesto, permisos y API del núcleo.
- Un módulo de prueba que se instala y desinstala sin tocar el núcleo.
- **Terminado cuando**: el módulo de prueba se instala, pide permisos, funciona y se desinstala limpiamente.

### Fase 8: Módulos de utilidad
- Notas, apps abiertas y fijadas, portapapeles, prueba de velocidad.
- **Terminado cuando**: cada módulo funciona como módulo instalable.

### Fase 9: Módulo de pareja
- Backend de sincronización, avatar animado, envío en tiempo real.
- **Terminado cuando**: dos instancias se envían mensajes con delay razonable.

### Fase 10: Instalación
- Arranque con Windows, instalador, actualizaciones.
- **Terminado cuando**: la app arranca sola, se instala con un solo archivo y se actualiza.

### Fase 11: Buscador de aplicaciones (hecha en la versión 2.0.0)
- Atajo global, ventana de búsqueda junto a la barra, resultados con cristal y animaciones del panel.
- Aplicaciones instaladas y abiertas, redirecciones y búsquedas web configurables desde el panel.
- Orden por coincidencia y uso reciente; Enter abre y ctrl+Enter abre como administrador.
- **Terminado cuando**: Erick puede desinstalar Flow Launcher y hacer con Tesela todo lo que hacía con él (sección 18.12).
- El detalle completo está en la sección 18.

---

## 13. Pruebas por caso límite

Estas pruebas se escriben desde la fase 2, sobre el dominio, sin abrir ventanas reales.

### Layout
- Agregar 1, 2, 5 y 10 columnas: cada una ocupa el ancho correcto.
- Redimensionar una columna: las demás se reajustan en proporción y la suma no cambia.
- Redimensionar hasta el mínimo: no baja de ahí.
- Cerrar una columna: las demás se reparten de nuevo.
- Cerrar la última columna: el workspace queda vacío sin errores.
- Mover una ventana a otra columna: ambas columnas se reajustan.
- Mover una ventana a otro workspace: sale de uno y entra en el otro.
- Agregar ventanas apiladas en una columna: el alto se reparte entre ellas.
- Redimensionar alto dentro de una columna: las demás se ajustan en proporción.

### Monitores
- Dos monitores, uno horizontal y uno vertical: cada uno calcula su layout por separado.
- Mover un workspace de un monitor a otro: su estructura se conserva.
- Desconectar un monitor con ventanas: las ventanas se reasignan al monitor principal.

### Exclusiones y pantalla completa
- Una ventana excluida no cambia de posición ni de tamaño.
- Cerrar una ventana excluida: no deja estado residual.
- Una ventana en pantalla completa oculta la barra en su monitor.
- Salir de pantalla completa: la barra reaparece.
- Dos ventanas en pantalla completa en monitores distintos: cada barra se oculta en su monitor.

### Atajos
- Cada atajo de la sección 8 ejecuta la acción asignada.
- Dos atajos con la misma combinación: la app lo reporta y no carga ninguno hasta resolverlo.
- Recargar configuración con atajos inválidos: conserva los anteriores y avisa.

### Módulos
- Instalar un módulo que pide un permiso no concedido: no accede a esa función.
- Desinstalar un módulo: no deja procesos ni suscripciones activas.
- Un módulo que falla: no tumba el núcleo.

---

## 14. Herramientas

| Área | Herramienta | Notas |
|---|---|---|
| Base de escritorio | Tauri 2 | Núcleo en Rust, interfaz web. |
| Lenguaje del núcleo | Rust | Acceso a Win32 y WinRT. |
| API de Windows | `windows-rs` | Ventanas, monitores, DWM, energía. |
| Métricas del sistema | `sysinfo` | CPU, RAM, disco, red. |
| Tareas asíncronas | `tokio` | Sin bloquear la interfaz. |
| Configuración | `serde` | Lectura y escritura de JSON o TOML. |
| Frontend | Svelte (recomendado) | Compila a poco JavaScript. |
| Prueba de velocidad | Ookla CLI | Ejecutado en segundo plano. |
| Notas | Markdown en carpeta local | Sin base de datos. |
| Módulo de pareja | Supabase | Canales en tiempo real. |
| Pruebas del dominio | `cargo test` | Sin ventanas reales. |
| Pruebas de interfaz | Vitest | Componentes del frontend. |

---

## 15. Riesgos

- **Reimplementar un gestor de ventanas** es el trabajo más grande. Hay muchos casos raros (diálogos, pantallas completas, juegos). Mitigación: exclusiones desde la interfaz y pruebas con apps reales.
- **Actualizaciones de Windows 11** pueden cambiar comportamientos no documentados. Mitigación: GlazeWM como respaldo y pruebas después de cada actualización.
- **Bluetooth** es lo más complejo de los módulos, por la API WinRT. Mitigación: dejarlo para una fase tardía.
- **Permisos de administrador** dan acceso amplio. Mitigación: los módulos solo reciben los permisos declarados en su manifiesto.
- **Módulo de pareja** depende de un backend. Mitigación: se desarrolla al final, cuando el resto esté estable.
- **Buscador**: el atajo ctrl+shift+espacio lo usan otras aplicaciones (por ejemplo, el editor VS Code lo usa para mostrar la ayuda de parámetros); al registrarse como atajo global, Tesela se lo quitará a todas. Mitigación: el atajo es configurable en el archivo de configuración y se avisa si no se puede registrar (ver 18.4).
- **Buscador**: la ventana de búsqueda necesita foco de teclado y, con un WebView de Tesela enfocado, Windows deja de llamar a los ganchos de teclado. Mitigación ya conocida: ver 18.4.

---

## 16. Configuración de referencia

El archivo `config.yaml` actual es la referencia para la migración. Conceptos que se conservan:

- Gaps interiores y exteriores.
- Bordes de ventanas: enfocada y no enfocada, con colores.
- Workspaces ligados a monitores.
- Reglas de ventanas: mover a workspace, flotante centrado, ignorar.
- Atajos y modo redimensionar.
- Arranque de aplicaciones: Zebar (se elimina, lo reemplaza la app), script de Python (fuera de alcance), Spotify, Discord.

Los conceptos que cambian:

- Las reglas de **ignorar** pasan a ser exclusiones seleccionables desde la interfaz.
- Las reglas de **mover a workspace** se conservan, pero también se gestionan desde la interfaz.
- Zebar deja de existir como dependencia.

---

## 17. Glosario

- **Columna**: contenedor vertical que ocupa una parte del ancho del workspace.
- **Ventana apilada**: ventana que comparte una columna con otras, repartiéndose el alto.
- **Workspace**: conjunto de ventanas organizado en columnas, asignado a un monitor.
- **Exclusión**: ventana que la app no gestiona.
- **Cloak**: técnica de DWM para ocultar ventanas sin minimizarlas.
- **AppBar**: franja de Windows que reserva espacio en el borde de la pantalla.
- **Módulo**: funcionalidad instalable que usa la API del núcleo.
- **Manifiesto**: archivo que describe un módulo y sus permisos.
- **Redirección**: palabra que, escrita en el buscador, abre una dirección web fija (por ejemplo, «claude» abre claude.com).
- **Búsqueda web**: palabra clave seguida de un texto que se envía a una página de búsqueda (por ejemplo, «g gatos»).

---

## 18. Buscador de aplicaciones propio (implementado en la versión 2.0.0)

> **Estado: implementado en la versión 2.0.0.** Esta sección conserva lo acordado; las decisiones que quedaban «a confirmar»
> se cerraron al empezar (ver 18.14) y lo que se hizo está en D-106 de DECISIONS.md.

### 18.1 Objetivo

Tener dentro de Tesela un buscador rápido de aplicaciones, al estilo de **Flow Launcher**, para poder **desinstalar Flow
Launcher** y dejar una sola herramienta. Se abre con un atajo global, se escribe, se elige con el teclado y se abre.

### 18.2 Decisiones de Erick

| Tema | Decisión |
|---|---|
| Atajo | **ctrl+shift+espacio** (abre y también cierra el buscador). |
| Qué busca | **Aplicaciones** (instaladas y abiertas) y **redirecciones / búsquedas web** configurables. Sin archivos, sin calculadora, sin comandos del sistema en la primera versión. |
| Redirecciones | Se configuran desde el **panel informativo de Tesela** (Ajustes). Ejemplo pedido: escribir un nombre y abrir `claude.com`. |
| Aspecto | **Ventana pequeña junto a la barra**, con los **resultados debajo**, con el **mismo estilo de cristal y animaciones del panel**. Similar a Flow Launcher. |
| Enter | Abre el resultado normalmente. |
| ctrl+Enter | Abre el resultado **como administrador**. |
| Orden | **Por coincidencia y por uso reciente**: lo que más se abre, sale primero. |

### 18.3 Comportamiento

- **Abrir**: ctrl+shift+espacio muestra la ventana en el **monitor donde está el cursor**, con el cuadro de texto enfocado
  y vacío. Si ya está abierta, el mismo atajo la cierra.
- **Cerrar**: Escape, ctrl+shift+espacio otra vez, o perder el foco (hacer clic fuera). Al cerrarse con una animación
  igual que la del panel (0,38 s; la de apertura, 0,5 s).
- **Resultados en vivo** debajo del cuadro, mientras se escribe. Máximo de resultados visibles: unos 8; si hay más, se
  puede desplazar (con la barra de desplazamiento oculta pero funcional, como en la lista de segundo plano).
- **Teclado**: flechas arriba/abajo para moverse, Enter para abrir, ctrl+Enter para abrir como administrador, Escape para
  cerrar. El primer resultado queda seleccionado de forma automática.
- **Ratón**: clic en un resultado lo abre; pasar el cursor lo resalta.
- Cada resultado muestra **icono**, **nombre** y una línea secundaria (por ejemplo «Aplicación», «En ejecución», o la
  dirección de una redirección).
- Abrir una aplicación **que ya está abierta** la enfoca (cambiando de workspace si hace falta) en lugar de lanzar otra
  copia; si no está abierta, la lanza.
- Tras abrir algo, la ventana se cierra.

### 18.4 Atajo, foco y la tecla Windows

Puntos técnicos que condicionan el diseño y ya se conocen por el trabajo hecho con la barra y el panel:

- **Registro del atajo**: como atajo global del sistema (el mismo mecanismo de los atajos de la sección 8, que acepta
  `ctrl` y `shift`), **no** con el gancho de teclado: así no depende de que el gancho reciba teclas. Se declara en
  `config.toml` como un atajo más (`buscador` → `ctrl+shift+space`), por lo que es configurable. Si otra aplicación ya lo
  tiene registrado, se avisa y se explica cómo cambiarlo.
- **Aviso sobre el atajo**: ctrl+shift+espacio se usa en otras aplicaciones; al ser global, Tesela se lo quita (ver riesgos,
  sección 15).
- **Foco de teclado**: a diferencia de la barra de tareas, **esta ventana necesita foco** (hay que escribir). Se sabe que,
  con un WebView de Tesela con foco de teclado, Windows deja de llamar a los ganchos de teclado y la tecla Windows podría
  abrir la búsqueda de Windows. Por eso la ventana de búsqueda debe llevar **el mismo respaldo que el panel**: avisar al
  núcleo de las teclas que le llegan para que la tecla Windows siga siendo de Tesela, y reinstalar el gancho mientras esté
  abierta.
- **Cierre al perder el foco**, como el panel; el clic que la cierra no debe reabrirla.
- Se reutiliza la ventana (se oculta, no se destruye) y su WebView se duerme mientras está oculta.

### 18.5 Fuentes de resultados

1. **Aplicaciones instaladas**: las del menú Inicio (accesos directos y aplicaciones de la tienda). Ya existe un escáner de
   aplicaciones instaladas en el núcleo, que se reutiliza y se amplía (nombre visible, ruta o identificador, icono).
   Se indexa al arrancar y se actualiza cada cierto tiempo y cada vez que se abre el buscador si pasó un rato.
2. **Aplicaciones abiertas**: las ventanas actuales (las mismas que ve la barra de tareas). Aparecen marcadas «En
   ejecución» y, al elegirlas, se enfocan.
3. **Redirecciones**: una palabra que abre una dirección fija (18.6).
4. **Búsquedas web**: una palabra clave más un texto, que se envía a un buscador (18.6).

Las fuentes se consultan en el núcleo (Rust), no en la interfaz; la interfaz solo recibe la lista ya ordenada.

### 18.6 Redirecciones y búsquedas web (configurables)

Se administran desde el **panel informativo → Ajustes → Buscador** (sección nueva). Cada entrada tiene:

| Campo | Ejemplo | Para qué |
|---|---|---|
| Palabra | `claude` | Lo que se escribe para activarla. |
| Dirección | `https://claude.com` | A dónde lleva. |
| Tipo | Redirección / Búsqueda | Una redirección abre la dirección tal cual; una búsqueda sustituye el texto escrito. |

- **Redirección**: al escribir `claude`, aparece el resultado «Abrir claude.com»; Enter abre la dirección en el navegador
  predeterminado.
- **Búsqueda web** (suposición, a confirmar): la dirección lleva el marcador `{q}`, por ejemplo
  `https://www.google.com/search?q={q}`; al escribir `g gatos` se abre la búsqueda de «gatos». El texto se codifica para
  direcciones web antes de insertarlo.
- Se pueden **añadir, editar, borrar y reordenar** desde el panel; se guardan en un archivo de la carpeta de configuración
  (por ejemplo `search.json`), igual que las exclusiones.
- **Entradas de fábrica (a confirmar)**: ninguna redirección; una búsqueda de ejemplo con `g` (Google) que Erick puede
  borrar.
- **Reglas de validación**: solo direcciones `http://` y `https://`; la palabra no puede estar vacía ni repetirse; un
  texto con `{q}` solo en las de tipo búsqueda.
- Coincidencia: se compara la palabra al **principio** de lo escrito; si coincide exactamente con una redirección, esta
  sube a primer lugar.

### 18.7 Orden de los resultados

Puntuación combinada, calculada en el núcleo:

1. **Coincidencia** con lo escrito: empieza por el texto > palabras que empiezan por él > letras en orden (búsqueda
   «difusa»). Sin coincidencia no aparece.
2. **Uso reciente**: cada vez que se abre un resultado desde el buscador se anota; los más abiertos y los abiertos hace
   poco suben. El peso del uso envejece con el tiempo para que lo antiguo no domine.
3. **Desempate**: las aplicaciones en ejecución primero, después por orden alfabético.
4. Con el cuadro vacío se muestran las **más usadas** (hasta unas 8).

El historial de uso se guarda **localmente** en un archivo de la carpeta de estado y se puede **borrar** desde
Ajustes → Buscador. No se envía a ningún sitio.

### 18.8 Abrir como administrador (ctrl+Enter)

- Se lanza el programa pidiendo elevación a Windows; Windows muestra su aviso de control de cuentas (UAC) y el usuario lo
  acepta o no. Tesela **no** guarda ni salta ese aviso.
- Solo tiene sentido para aplicaciones; en una redirección o una búsqueda web, ctrl+Enter hace lo mismo que Enter.
- Si el usuario cancela el aviso, no pasa nada (no se muestra error).

### 18.9 Arquitectura (por capas)

- **Dominio** (puro, con pruebas): modelo de resultado (`SearchItem`: tipo, título, subtítulo, destino, puntuación),
  puntuación y orden (`score`, `rank`), análisis de lo escrito (¿redirección?, ¿búsqueda con palabra clave?), validación
  de redirecciones, normalización del uso.
- **Aplicación**: caso de uso «buscar» (une las fuentes y ordena) y «abrir» (enfocar si está abierta, lanzar si no,
  elevar si se pide, anotar el uso).
- **Infraestructura**: índice de aplicaciones instaladas (ampliando el escáner existente), lectura de ventanas abiertas,
  almacén de redirecciones y almacén de uso (JSON en disco), lanzamiento (con y sin elevación) y apertura de direcciones
  web con el navegador predeterminado, extracción de iconos por ejecutable (ya existe).
- **Interfaz**: ventana propia sin decoración y transparente (`search`), comandos `search_query(texto)`,
  `search_open(id, como_admin)`, `search_close`, `search_redirects_get/set`, `search_history_clear`; vista Svelte con
  cuadro de texto y lista de resultados; sección nueva en Ajustes. Los comandos reciben **identificadores de resultado**,
  nunca rutas ni direcciones: el núcleo las resuelve (misma regla de seguridad que las acciones de segundo plano).

### 18.10 Seguridad

- La interfaz **nunca manda rutas ni comandos**: manda el identificador de un resultado que el propio núcleo acaba de
  ofrecer. Si el identificador no existe o ya no es válido, no se hace nada.
- Las direcciones de las redirecciones solo pueden ser `http://` o `https://`; se rechazan `file:`, `javascript:` y
  esquemas desconocidos.
- Lo escrito en una búsqueda web se **codifica** antes de insertarse en la dirección; nunca se pasa por una consola.
- Abrir como administrador siempre pasa por el aviso de Windows.

### 18.11 Pruebas previstas

- **Dominio**: la puntuación ordena bien (empieza por, palabras, difusa); el uso reciente sube un resultado pero no uno
  que no coincide; el envejecimiento funciona; el análisis de «palabra + texto»; la validación de redirecciones (esquemas
  y duplicados); la codificación de `{q}` con tildes, espacios y símbolos.
- **Infraestructura**: el almacén de redirecciones y el de uso leen y guardan; un archivo dañado no rompe nada; el índice
  de aplicaciones encuentra al menos una aplicación real (prueba en vivo `#[ignore]`).
- **Interfaz**: la lista se mueve con las flechas; Enter y ctrl+Enter llaman a la acción correcta; Escape cierra; el primer
  resultado se selecciona solo; la sección de Ajustes añade, edita y borra redirecciones.
- **En vivo** (con teclas simuladas, como las demás): el atajo abre y cierra la ventana en el monitor del cursor; abrir una
  aplicación ya abierta la enfoca; la tecla Windows sigue siendo de Tesela con el buscador abierto.

### 18.12 Terminado cuando

Erick puede desinstalar Flow Launcher y: abrir cualquier aplicación escribiendo unas letras, enfocar las que ya están
abiertas, abrir una como administrador, usar sus redirecciones (como la de `claude.com`), y todo ello con el atajo
ctrl+shift+espacio, sin que la tecla Windows ni el resto de Tesela se vean afectados.

### 18.13 Fuera de alcance (por ahora)

Búsqueda de archivos y carpetas, calculadora y conversiones, comandos del sistema (apagar, bloquear…), portapapeles,
complementos de terceros y sincronización de las redirecciones entre equipos. Todo eso puede añadirse después como
fuentes nuevas sin cambiar la arquitectura de 18.9.

### 18.14 Detalles que se confirmaron al empezar

**Respuestas de Erick:** (1) sin búsquedas web de fábrica: `search.json` empieza vacío y todo se añade desde Ajustes → Buscador;
(2) centrada bajo la barra superior del monitor del cursor, como el panel; (3) Flow Launcher puede convivir mientras tanto;
(4) el atajo es `ctrl+shift+espacio` y, si choca, se cambia en `config.toml`; (5) **sin aplicaciones de la Tienda de Microsoft
(UWP)**: solo los accesos directos del menú Inicio que apuntan a un `.exe`.

Las preguntas originales eran:

1. **Búsquedas web**: formato con `{q}` y la palabra clave seguida de un espacio (suposición de 18.6). ¿Se quiere alguna
   de fábrica además de Google?
2. **Posición exacta**: centrada bajo la barra superior del monitor del cursor, como el panel, o alineada a otro punto.
3. **Qué hacer con Flow Launcher** mientras tanto: puede convivir (no usan el mismo atajo si el de Flow sigue siendo
   alt+espacio). Erick lo desinstalará cuando el buscador cumpla 18.12.
4. **Atajo**: si ctrl+shift+espacio choca con otra aplicación que Erick use, ¿se prefiere otro?
