# Módulo «Pareja»: estado y plan para conectar el servidor (Fase 9)

## Qué está hecho

- **Módulo `modules/partner`** (instalable como cualquier otro): mensajes, botones rápidos (❤️ 😘 👋 🥰), avatar animado
  (se anima al enviar y al recibir, parpadea en reposo, respeta `prefers-reduced-motion`), historial en archivo, ajustes
  (tu nombre y el **código de pareja**), hueco pequeño en la barra con un punto cuando llegan mensajes nuevos.
- **Canales en el núcleo** (§10.4 «enviar mensaje a un canal»): API `channels.info/subscribe/unsubscribe/send` con el permiso
  `channels`, validación de nombres, tope de tamaño (2000 caracteres) y de ritmo (20 envíos cada 10 s), y entrega solo
  a módulos suscritos que sigan teniendo el permiso.
- **Backend de canales local y simulado** (`LocalChannelBackend`): entrega dentro de esta app con un retardo de 400 ms que
  imita la red. Hay una «pareja virtual» opcional que responde sola para poder ver el avatar sin una segunda persona.
- **Instalación separada** (§10.6): la app solo gestiona ventanas si se arranca con `--manage`. Sin esa opción es un
  anfitrión de barra, panel y módulos, que es lo que necesita tu pareja: instala la app y este módulo, nada más.

## Qué falta (necesita tu aprobación y tus credenciales)

1. **Un proyecto de Supabase** (no lo creé: tus reglas lo prohíben sin tu confirmación) con Realtime activado.
2. **`RemoteChannelBackend`** en `src-tauri/src/infrastructure/`: implementa `ChannelBackend` (ya definido en
   `application/channels.rs`) contra Supabase Realtime. No está escrito porque no se puede probar sin servidor; el módulo y
   la API de canales **no cambian** cuando se enchufe (el módulo ya detecta `channels.info().backend`).
3. **Configuración**: `[sync] url`, `anon_key` en `config.toml`; el backend remoto se elige al arrancar si están presentes.
4. **Autenticación**: cada persona inicia sesión (propuesta: enlace mágico por correo o Auth anónimo + código de pareja).
5. **Autorización por pareja**: Realtime con canales privados y una política sobre `realtime.messages` que solo deje
   entrar a los dos miembros de la pareja.
6. **Decisiones de diseño tuyas**: el avatar (el actual es un marcador de posición), qué notificar y si los mensajes se
   guardan en el servidor o solo viajan (propuesta: solo viajan; el historial queda en cada equipo).

## Protocolo propuesto (Supabase Realtime, canal *Broadcast*, sin tablas para los mensajes)

- WebSocket: `wss://<ref>.supabase.co/realtime/v1/websocket?apikey=<anon_key>&vsn=1.0.0`
- Unirse: `{"topic":"realtime:<canal>","event":"phx_join","payload":{"config":{"broadcast":{"self":false,"ack":true},"private":true},"access_token":"<jwt>"},"ref":"1"}`
- Enviar: `{"topic":"realtime:<canal>","event":"broadcast","payload":{"type":"broadcast","event":"msg","payload":{...}},"ref":"n"}`
- Latido cada 25 s: `{"topic":"phoenix","event":"heartbeat","payload":{},"ref":"n"}`; reconexión con espera creciente.
- Política de ejemplo (a revisar contigo antes de ejecutar nada):

```sql
create table couples (id uuid primary key default gen_random_uuid(), a uuid not null, b uuid not null);
create policy "pareja puede leer su canal" on realtime.messages for select to authenticated
  using (exists (select 1 from couples c where (c.a = auth.uid() or c.b = auth.uid()) and realtime.topic() = 'realtime:' || c.id::text));
create policy "pareja puede escribir en su canal" on realtime.messages for insert to authenticated
  with check (exists (select 1 from couples c where (c.a = auth.uid() or c.b = auth.uid()) and realtime.topic() = 'realtime:' || c.id::text));
```

## Cómo se prueba cuando exista el servidor

Dos instancias reales (dos equipos o dos usuarios de Windows) con el mismo código de pareja: cada una envía y la otra recibe en
un tiempo razonable (objetivo: menos de 2 s). La prueba automatizada actual (`tests/phase9_partner.rs`) cubre el camino
completo con el canal local; la de verdad necesita el servidor.
