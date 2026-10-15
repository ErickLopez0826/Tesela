/** Tamaño en bytes con la unidad que mejor se lee. */
export function formatBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = Math.max(0, bytes);
  let i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i++;
  }
  return `${i === 0 ? Math.round(value) : value.toFixed(1)} ${units[i]}`;
}

/** Velocidad de red en bytes por segundo. */
export function formatRate(bytesPerSecond: number): string {
  return `${formatBytes(bytesPerSecond)}/s`;
}

/** Tiempo encendido: "5 min", "2 h 37 min", "3 d 4 h". */
export function formatUptime(secs: number): string {
  const minutes = Math.floor(secs / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);
  if (days > 0) return `${days} d ${hours % 24} h`;
  if (hours > 0) return `${hours} h ${minutes % 60} min`;
  return `${minutes} min`;
}

export function percentOf(used: number, total: number): number {
  return total > 0 ? Math.min(100, (used / total) * 100) : 0;
}
