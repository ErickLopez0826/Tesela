import { describe, expect, it } from "vitest";
import { formatMetric, rankProcesses } from "./processes";

const p = (name: string, cpu: number, ram: number, gpu: number, disk: number) => ({ name, cpu_percent: cpu, ram_bytes: ram, gpu_percent: gpu, disk_bytes_per_sec: disk });
const list = [p("a.exe", 5, 100, 0, 0), p("b.exe", 40, 50, 12, 1000), p("c.exe", 1, 900, 80, 5000)];

describe("apps que más consumen", () => {
  it("ordena por el criterio elegido", () => {
    expect(rankProcesses(list, "ram").map((x) => x.name)).toEqual(["c.exe", "a.exe", "b.exe"]);
    expect(rankProcesses(list, "cpu").map((x) => x.name)).toEqual(["b.exe", "a.exe", "c.exe"]);
    expect(rankProcesses(list, "gpu").map((x) => x.name)).toEqual(["c.exe", "b.exe"]);
    expect(rankProcesses(list, "disk").map((x) => x.name)).toEqual(["c.exe", "b.exe"]);
  });

  it("respeta el límite y no modifica la lista original", () => {
    expect(rankProcesses(list, "ram", 1).length).toBe(1);
    expect(list[0].name).toBe("a.exe");
  });

  it("da formato a cada magnitud", () => {
    expect(formatMetric(12.4, "cpu")).toBe("12%");
    expect(formatMetric(2.34, "gpu")).toBe("2.3%");
    expect(formatMetric(2048, "disk")).toBe("2.0 KB/s");
    expect(formatMetric(1024 ** 3, "ram")).toBe("1.0 GB");
  });
});
