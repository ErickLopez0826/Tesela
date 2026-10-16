import type { ProcessUsage } from "../api/system";
import { formatBytes, formatRate } from "./format";

export type ProcessMetric = "ram" | "cpu" | "gpu" | "disk";

export const METRICS: { id: ProcessMetric; label: string }[] = [
  { id: "ram", label: "RAM" },
  { id: "cpu", label: "CPU" },
  { id: "gpu", label: "GPU" },
  { id: "disk", label: "Disco" },
];

export function metricValue(p: ProcessUsage, metric: ProcessMetric): number {
  switch (metric) {
    case "ram": return p.ram_bytes;
    case "cpu": return p.cpu_percent;
    case "gpu": return p.gpu_percent ?? 0;
    case "disk": return p.disk_bytes_per_sec ?? 0;
  }
}

export function formatMetric(value: number, metric: ProcessMetric): string {
  switch (metric) {
    case "ram": return formatBytes(value);
    case "disk": return formatRate(value);
    default: return `${value < 10 && value > 0 ? value.toFixed(1) : Math.round(value)}%`;
  }
}

/** Las apps que más consumen según el criterio elegido (las que no consumen nada de eso no aparecen). */
export function rankProcesses(list: ProcessUsage[], metric: ProcessMetric, limit = 5): ProcessUsage[] {
  return list
    .filter((p) => metricValue(p, metric) > 0)
    .sort((a, b) => metricValue(b, metric) - metricValue(a, metric))
    .slice(0, limit);
}
