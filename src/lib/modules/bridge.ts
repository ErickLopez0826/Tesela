/** Puente entre un módulo (iframe aislado) y el núcleo. Protocolo de mensajes `{ wm: 1, ... }`. */

export interface RequestMessage {
  wm: 1;
  id: number;
  method: string;
  params: unknown;
}

export type ReplyMessage = { wm: 1; id: number; result: unknown } | { wm: 1; id: number; error: string };

export const MAX_METHOD_LENGTH = 64;

/** ¿Es un mensaje de petición bien formado? Todo lo demás se ignora sin responder. */
export function parseRequest(data: unknown): RequestMessage | null {
  if (typeof data !== "object" || data === null) return null;
  const m = data as Record<string, unknown>;
  if (m.wm !== 1 || typeof m.id !== "number" || !Number.isInteger(m.id)) return null;
  if (typeof m.method !== "string" || m.method.length === 0 || m.method.length > MAX_METHOD_LENGTH) return null;
  return { wm: 1, id: m.id, method: m.method, params: m.params ?? null };
}

export type CallCore = (module: string, method: string, params: unknown) => Promise<unknown>;

export interface BridgeRecord {
  module: string;
  method: string;
  params?: unknown;
  ok: boolean;
  /** Texto del error cuando `ok` es falso (p. ej. "permission_denied: ..."). */
  error?: string;
}

/**
 * Atiende las peticiones de UN módulo. El id del módulo lo fija el anfitrión (según el iframe que
 * envió el mensaje), nunca el propio módulo: así un módulo no puede hacerse pasar por otro.
 */
export function createBridge(module: string, callCore: CallCore, onRecord?: (r: BridgeRecord) => void) {
  return async function handle(data: unknown): Promise<ReplyMessage | null> {
    const req = parseRequest(data);
    if (!req) return null;
    try {
      const result = await callCore(module, req.method, req.params);
      onRecord?.({ module, method: req.method, params: req.params, ok: true });
      return { wm: 1, id: req.id, result: result ?? null };
    } catch (e) {
      const error = e instanceof Error ? e.message : String(e);
      onRecord?.({ module, method: req.method, params: req.params, ok: false, error });
      return { wm: 1, id: req.id, error };
    }
  };
}

export const eventMessage = (event: string, payload: unknown) => ({ wm: 1, event, payload });
export const actionMessage = (action: string) => ({ wm: 1, action });
