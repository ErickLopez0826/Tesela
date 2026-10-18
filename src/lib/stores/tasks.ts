import type { Priority, Task } from "../api/tasks";

export const PRIORITIES: Priority[] = ["high", "medium", "low"];
export const PRIORITY_LABEL: Record<Priority, string> = { high: "Alta", medium: "Media", low: "Baja" };
export const MAX_TASK_CHARS = 200;

function makeId(): string {
  return globalThis.crypto?.randomUUID?.() ?? `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}

/** Una tarea nueva con el texto limpio (sin saltos de línea ni espacios de más); `null` si no hay texto. */
export function newTask(text: string, priority: Priority, id: string = makeId()): Task | null {
  const clean = text.split(/\s+/).filter(Boolean).join(" ").slice(0, MAX_TASK_CHARS);
  return clean ? { id, text: clean, priority } : null;
}

/** Sube (`-1`) o baja (`1`) una tarea un lugar. Si no puede moverse, devuelve la misma lista. */
export function move(tasks: Task[], id: string, delta: -1 | 1): Task[] {
  const from = tasks.findIndex((t) => t.id === id);
  const to = from + delta;
  if (from < 0 || to < 0 || to >= tasks.length) return tasks;
  const next = [...tasks];
  [next[from], next[to]] = [next[to], next[from]];
  return next;
}

/** Alta → media → baja → alta. */
export function cyclePriority(p: Priority): Priority {
  return PRIORITIES[(PRIORITIES.indexOf(p) + 1) % PRIORITIES.length];
}

export function setPriority(tasks: Task[], id: string, priority: Priority): Task[] {
  return tasks.map((t) => (t.id === id ? { ...t, priority } : t));
}

/** Quita una tarea (al marcarla como hecha). */
export function without(tasks: Task[], id: string): Task[] {
  return tasks.filter((t) => t.id !== id);
}

/** Las de prioridad alta primero y las bajas al final; dentro de cada prioridad se conserva el orden que ya tenían. */
export function sortByPriority(tasks: Task[]): Task[] {
  const rank = (t: Task) => PRIORITIES.indexOf(t.priority);
  return tasks.map((t, i) => ({ t, i })).sort((a, b) => rank(a.t) - rank(b.t) || a.i - b.i).map(({ t }) => t);
}
