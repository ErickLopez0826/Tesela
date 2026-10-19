/** Intentos fallidos de cargar el logo de cada página: así una página sin logo no se vuelve a probar en cada búsqueda. */
const failures = new Map<string, number>();

export const failedCount = (site: string): number => failures.get(site) ?? 0;
export const rememberFailed = (site: string, count: number): void => void failures.set(site, count);
/** Solo para las pruebas. */
export const clearFavicons = (): void => failures.clear();

/** ¿Es un servidor de internet (con nombre de dominio público)? Los de la red local no se piden a ningún servicio de fuera. */
export function isPublicHost(host: string): boolean {
  const h = host.toLowerCase();
  if (!h.includes(".") || /^[\d.]+$/.test(h) || h.includes(":")) return false; // sin punto, IPv4 o IPv6
  return !/\.(local|lan|internal|localhost|home|corp)$/.test(h);
}

/**
 * De dónde se puede sacar el logo de una página, en orden. `site` es el origen (`https://claude.ai`) que da el núcleo.
 * 1. El `favicon.ico` de la propia página: solo se habla con la página que tú configuraste.
 * 2. Si no lo tiene, el servicio de iconos de Google, que recibe solo el nombre del servidor (nunca lo que escribes).
 *    No se usa con servidores locales.
 */
export function faviconSources(site: string): string[] {
  let url: URL;
  try {
    url = new URL(site);
  } catch {
    return [];
  }
  if (url.protocol !== "https:" && url.protocol !== "http:") return [];
  const sources = [`${url.origin}/favicon.ico`];
  if (isPublicHost(url.hostname)) {
    sources.push(`https://www.google.com/s2/favicons?domain=${encodeURIComponent(url.hostname)}&sz=64`);
  }
  return sources;
}
