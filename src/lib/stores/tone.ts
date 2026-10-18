/** Luminosidad (0 a 1) del tinte oscuro del panel y la barra: rgba(10, 10, 11). */
export const TINT_LUMA = 0.04;
/** Por encima de esta luminosidad del fondo el texto pasa a ser oscuro. */
export const LIGHT_BACKGROUND = 0.55;

export interface Tone {
  fg: string;
  muted: string;
}

const LIGHT_TEXT: Tone = { fg: "#f1f1f1", muted: "#8b8b90" };
const DARK_TEXT: Tone = { fg: "#111114", muted: "#55555c" };

/**
 * Luminosidad del fondo que realmente se ve: el tinte oscuro (con su opacidad `alpha`, 0 a 1) sobre lo que haya detrás
 * (`behind`, la luminosidad de la copia desenfocada; sin ella se supone un fondo oscuro).
 */
export function effectiveLuma(behind: number | null, alpha: number): number {
  const a = Math.min(1, Math.max(0, alpha));
  return a * TINT_LUMA + (1 - a) * (behind ?? TINT_LUMA);
}

/** Colores de texto que se leen sobre un fondo de esa luminosidad: claros sobre oscuro y oscuros sobre claro. */
export function toneFor(luma: number): Tone {
  return luma > LIGHT_BACKGROUND ? DARK_TEXT : LIGHT_TEXT;
}

/** Luminosidad media (0 a 1) de una imagen, o `null` si no se puede leer. Se reduce a 16×16 antes de promediar. */
export function imageLuma(url: string | undefined | null): Promise<number | null> {
  if (!url) return Promise.resolve(null);
  return new Promise((resolve) => {
    try {
      const img = new Image();
      img.onload = () => {
        try {
          const canvas = document.createElement("canvas");
          canvas.width = canvas.height = 16;
          const ctx = canvas.getContext("2d");
          if (!ctx) return resolve(null);
          ctx.drawImage(img, 0, 0, 16, 16);
          const data = ctx.getImageData(0, 0, 16, 16).data;
          let sum = 0;
          for (let i = 0; i < data.length; i += 4) sum += (0.299 * data[i] + 0.587 * data[i + 1] + 0.114 * data[i + 2]) / 255;
          resolve(sum / (data.length / 4));
        } catch {
          resolve(null);
        }
      };
      img.onerror = () => resolve(null);
      img.src = url;
    } catch {
      resolve(null);
    }
  });
}
