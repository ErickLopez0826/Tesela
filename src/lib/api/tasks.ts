import { invoke } from "@tauri-apps/api/core";

export type Priority = "high" | "medium" | "low";

/** Una tarea: solo su nombre y su prioridad. El orden de la lista es el orden en que se hacen. */
export interface Task {
  id: string;
  text: string;
  priority: Priority;
}

export interface TasksApi {
  list: () => Promise<Task[]>;
  /** Guarda la lista completa (la valida) y devuelve la lista ya limpia. */
  save: (tasks: Task[]) => Promise<Task[]>;
}

export const tasksApi: TasksApi = {
  list: () => invoke<Task[]>("tasks_list"),
  save: (tasks) => invoke<Task[]>("tasks_save", { tasks }),
};
