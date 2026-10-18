import { invoke } from "@tauri-apps/api/core";
import { theme } from "./theme";

/**
 * Avisa al núcleo de si el borde de la ventana en uso está encendido («Sin marco» lo apaga): así, apagado, ni se muestra
 * ni se mueve la ventana del borde. La ventana del borde no puede avisar ella misma: oculta, su vista no corre.
 */
export function installBorderSync(): () => void {
  let last: boolean | null = null;
  return theme.subscribe((t) => {
    const on = t.borderStyle !== "off";
    if (on === last) return;
    last = on;
    try {
      void Promise.resolve(invoke("border_enable", { on })).catch(() => {});
    } catch {
      /* sin núcleo (pruebas) */
    }
  });
}
