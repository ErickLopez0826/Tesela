import { describe, expect, it } from "vitest";
import type { Task } from "../api/tasks";
import { MAX_TASK_CHARS, cyclePriority, move, newTask, setPriority, sortByPriority, without } from "./tasks";

const t = (id: string, priority: Task["priority"] = "medium"): Task => ({ id, text: `tarea ${id}`, priority });

describe("tareas", () => {
  it("una tarea nueva lleva solo nombre y prioridad, con el texto limpio", () => {
    expect(newTask("  comprar \n  pan ", "high", "x")).toEqual({ id: "x", text: "comprar pan", priority: "high" });
    expect(newTask("   \n ", "low")).toBeNull();
    expect(newTask("a".repeat(MAX_TASK_CHARS + 50), "low")!.text.length).toBe(MAX_TASK_CHARS);
    expect(newTask("a", "low")!.id).not.toBe(newTask("a", "low")!.id);
  });

  it("subir y bajar mueve un lugar y no se sale de los extremos", () => {
    const list = [t("a"), t("b"), t("c")];
    expect(move(list, "b", -1).map((x) => x.id)).toEqual(["b", "a", "c"]);
    expect(move(list, "b", 1).map((x) => x.id)).toEqual(["a", "c", "b"]);
    expect(move(list, "a", -1)).toBe(list);
    expect(move(list, "c", 1)).toBe(list);
    expect(move(list, "zzz", 1)).toBe(list);
    expect(list.map((x) => x.id)).toEqual(["a", "b", "c"]); // no se modifica la original
  });

  it("la prioridad pasa de alta a media a baja y vuelve a alta", () => {
    expect(["high", "medium", "low"].map((p) => cyclePriority(p as Task["priority"]))).toEqual(["medium", "low", "high"]);
    expect(setPriority([t("a"), t("b")], "b", "high").map((x) => x.priority)).toEqual(["medium", "high"]);
  });

  it("marcar como hecha quita la tarea", () => {
    expect(without([t("a"), t("b")], "a").map((x) => x.id)).toEqual(["b"]);
    expect(without([t("a")], "zzz").length).toBe(1);
  });

  it("ordenar por prioridad pone las altas primero y respeta el orden dentro de cada una", () => {
    const list = [t("a", "low"), t("b", "high"), t("c", "medium"), t("d", "high"), t("e", "low")];
    expect(sortByPriority(list).map((x) => x.id)).toEqual(["b", "d", "c", "a", "e"]);
  });
});
