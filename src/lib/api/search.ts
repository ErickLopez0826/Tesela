import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Qué es un resultado: una app instalada, una app abierta (se enfoca), una redirección o una búsqueda web. */
export type SearchKind = "app" | "running" | "redirect" | "web_search";

/** Un resultado. El `id` es opaco: solo el núcleo sabe a qué corresponde (la interfaz nunca manda rutas ni direcciones). */
export interface SearchItem {
  id: string;
  kind: SearchKind;
  title: string;
  subtitle: string;
  /** Origen de la página (`https://claude.ai`) de una redirección o búsqueda web: de él sale su logo. Las apps no lo tienen. */
  site?: string | null;
}

export type RedirectKind = "redirect" | "search";

/** Una redirección (abre una dirección) o una búsqueda web (la dirección lleva `{q}`). */
export interface Redirect {
  word: string;
  url: string;
  kind: RedirectKind;
}

/** Lo que la ventana del buscador pide al núcleo. */
export interface SearchApi {
  /** Resultados para lo escrito; con el cuadro vacío, las aplicaciones más usadas. */
  query: (text: string) => Promise<SearchItem[]>;
  /** Abre un resultado (Enter) o lo abre como administrador (ctrl+Enter). El núcleo cierra el buscador. */
  open: (id: string, asAdmin: boolean) => Promise<void>;
  /** Cierra el buscador (Escape) devolviendo el foco a la ventana anterior. */
  close: () => Promise<void>;
  /** Ajusta el alto (lógico) de la ventana al de su contenido. */
  resize: (height: number) => Promise<void>;
  /** Icono (URL de datos) de un resultado que es un programa. */
  icon: (id: string) => Promise<string | null>;
}

/** Lo que Ajustes pide al núcleo sobre el buscador. */
export interface SearchSettingsApi {
  redirectsGet: () => Promise<Redirect[]>;
  /** Guarda la lista completa (la valida) y devuelve la lista ya limpia. */
  redirectsSet: (list: Redirect[]) => Promise<Redirect[]>;
  /** Cuántas cosas recuerda el historial de uso. */
  historyLen: () => Promise<number>;
  /** Borra el historial de uso y devuelve cuántas entradas quedan. */
  historyClear: () => Promise<number>;
}

export const searchApi: SearchApi = {
  query: (text) => invoke<SearchItem[]>("search_query", { text }),
  open: (id, asAdmin) => invoke<void>("search_open", { id, asAdmin }),
  close: () => invoke<void>("search_close"),
  resize: (height) => invoke<void>("search_resize", { height }),
  icon: (id) => invoke<string | null>("search_icon", { id }),
};

export const searchSettingsApi: SearchSettingsApi = {
  redirectsGet: () => invoke<Redirect[]>("search_redirects_get"),
  redirectsSet: (list) => invoke<Redirect[]>("search_redirects_set", { list }),
  historyLen: () => invoke<number>("search_history_len"),
  historyClear: () => invoke<number>("search_history_clear"),
};

/** El buscador se acaba de mostrar (la ventana se reutiliza, así que no se vuelve a montar). */
export function onSearchShown(callback: () => void): Promise<UnlistenFn> {
  return listen("wm-search-shown", () => callback());
}

/** El buscador empieza a cerrarse: la vista reproduce su animación al revés y la ventana se oculta poco después. */
export function onSearchClosing(callback: () => void): Promise<UnlistenFn> {
  return listen("wm-search-closing", () => callback());
}
