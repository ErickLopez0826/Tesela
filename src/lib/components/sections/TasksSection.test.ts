import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Task, TasksApi } from "../../api/tasks";
import TasksSection from "./TasksSection.svelte";

const task = (id: string, text: string, priority: Task["priority"] = "medium"): Task => ({ id, text, priority });

function api(initial: Task[] = []) {
  const save = vi.fn(async (list: Task[]) => list);
  const a: TasksApi = { list: vi.fn().mockResolvedValue(initial), save };
  return { a, save };
}

const texts = (container: HTMLElement) => [...container.querySelectorAll(".task .text")].map((e) => e.textContent);
const lastSaved = (save: ReturnType<typeof vi.fn>) => save.mock.calls.at(-1)![0] as Task[];

beforeEach(() => vi.useFakeTimers({ shouldAdvanceTime: true }));
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("TasksSection", () => {
  it("muestra las tareas guardadas con su prioridad, o avisa de que no hay", async () => {
    const { a } = api([task("1", "Llamar al banco", "high"), task("2", "Regar las plantas", "low")]);
    const { container } = render(TasksSection, { api: a });
    await waitFor(() => expect(texts(container)).toEqual(["Llamar al banco", "Regar las plantas"]));
    expect(container.querySelector('.task[data-priority="high"] .prio')?.textContent?.trim()).toBe("Alta");
    expect(container.querySelector('.task[data-priority="low"] .prio')?.textContent?.trim()).toBe("Baja");
    cleanup();
    render(TasksSection, { api: api([]).a });
    expect(await screen.findByText("Sin tareas pendientes.")).toBeTruthy();
  });

  it("crea una tarea solo con su nombre y una prioridad, y la guarda", async () => {
    const { a, save } = api();
    const { container } = render(TasksSection, { api: a });
    await fireEvent.input(screen.getByLabelText("Nueva tarea"), { target: { value: "  Pagar la luz " } });
    await fireEvent.change(screen.getByLabelText("Prioridad de la nueva tarea"), { target: { value: "high" } });
    await fireEvent.click(screen.getByRole("button", { name: "Agregar" }));
    expect(texts(container)).toEqual(["Pagar la luz"]);
    expect(lastSaved(save)).toEqual([expect.objectContaining({ text: "Pagar la luz", priority: "high" })]);
    expect(Object.keys(lastSaved(save)[0]).sort()).toEqual(["id", "priority", "text"]);
    expect((screen.getByLabelText("Nueva tarea") as HTMLInputElement).value).toBe("");
  });

  it("no crea tareas vacías y Enter agrega la escrita", async () => {
    const { a, save } = api();
    const { container } = render(TasksSection, { api: a });
    expect((screen.getByRole("button", { name: "Agregar" }) as HTMLButtonElement).disabled).toBe(true);
    await fireEvent.input(screen.getByLabelText("Nueva tarea"), { target: { value: "Con Enter" } });
    await fireEvent.submit(screen.getByLabelText("Nueva tarea").closest("form")!);
    expect(texts(container)).toEqual(["Con Enter"]);
    expect(save).toHaveBeenCalledTimes(1);
  });

  it("las tareas nuevas van al final de la lista", async () => {
    const { a } = api([task("1", "Primera")]);
    const { container } = render(TasksSection, { api: a });
    await waitFor(() => expect(texts(container)).toEqual(["Primera"]));
    await fireEvent.input(screen.getByLabelText("Nueva tarea"), { target: { value: "Segunda" } });
    await fireEvent.click(screen.getByRole("button", { name: "Agregar" }));
    expect(texts(container)).toEqual(["Primera", "Segunda"]);
  });

  it("el orden se cambia con subir y bajar, y se guarda; los extremos no se pueden mover", async () => {
    const { a, save } = api([task("1", "A"), task("2", "B"), task("3", "C")]);
    const { container } = render(TasksSection, { api: a });
    await waitFor(() => expect(texts(container)).toEqual(["A", "B", "C"]));
    expect((screen.getByLabelText("Subir: A") as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByLabelText("Bajar: C") as HTMLButtonElement).disabled).toBe(true);
    await fireEvent.click(screen.getByLabelText("Bajar: A"));
    expect(texts(container)).toEqual(["B", "A", "C"]);
    await fireEvent.click(screen.getByLabelText("Subir: C"));
    expect(texts(container)).toEqual(["B", "C", "A"]);
    expect(lastSaved(save).map((t) => t.text)).toEqual(["B", "C", "A"]);
  });

  it("tocar la prioridad la cambia (alta → media → baja → alta) y se guarda", async () => {
    const { a, save } = api([task("1", "A", "high")]);
    render(TasksSection, { api: a });
    const prio = await screen.findByLabelText("Prioridad de A: Alta. Cambiar");
    await fireEvent.click(prio);
    expect(lastSaved(save)[0].priority).toBe("medium");
    await fireEvent.click(screen.getByLabelText("Prioridad de A: Media. Cambiar"));
    await fireEvent.click(screen.getByLabelText("Prioridad de A: Baja. Cambiar"));
    expect(lastSaved(save)[0].priority).toBe("high");
  });

  it("marcar como hecha la desvanece y después la elimina (y la guarda sin ella)", async () => {
    const { a, save } = api([task("1", "A"), task("2", "B")]);
    const { container } = render(TasksSection, { api: a });
    await waitFor(() => expect(texts(container)).toEqual(["A", "B"]));
    await fireEvent.click(screen.getByLabelText("Marcar como hecha: A"));
    expect(container.querySelector(".task.done .text")?.textContent).toBe("A");
    expect(save).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(600);
    expect(texts(container)).toEqual(["B"]);
    expect(lastSaved(save).map((t) => t.text)).toEqual(["B"]);
  });

  it("si se cambia de pestaña mientras se desvanece, la tarea hecha queda eliminada igualmente", async () => {
    const { a, save } = api([task("1", "A"), task("2", "B")]);
    const { container, unmount } = render(TasksSection, { api: a });
    await waitFor(() => expect(texts(container)).toEqual(["A", "B"]));
    await fireEvent.click(screen.getByLabelText("Marcar como hecha: A"));
    unmount();
    await vi.advanceTimersByTimeAsync(10);
    expect(lastSaved(save).map((t) => t.text)).toEqual(["B"]);
  });

  it("ordenar por prioridad pone las altas primero", async () => {
    const { a, save } = api([task("1", "baja", "low"), task("2", "alta", "high"), task("3", "media", "medium")]);
    const { container } = render(TasksSection, { api: a });
    await waitFor(() => expect(texts(container).length).toBe(3));
    await fireEvent.click(screen.getByRole("button", { name: "Ordenar por prioridad" }));
    expect(texts(container)).toEqual(["alta", "media", "baja"]);
    expect(lastSaved(save).map((t) => t.text)).toEqual(["alta", "media", "baja"]);
    expect((screen.getByRole("button", { name: "Ordenar por prioridad" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("si no se puede guardar, avisa y deja la lista como estaba", async () => {
    const save = vi.fn().mockRejectedValue("disco lleno");
    const { container } = render(TasksSection, { api: { list: vi.fn().mockResolvedValue([task("1", "A")]), save } });
    await waitFor(() => expect(texts(container)).toEqual(["A"]));
    await fireEvent.input(screen.getByLabelText("Nueva tarea"), { target: { value: "B" } });
    await fireEvent.click(screen.getByRole("button", { name: "Agregar" }));
    await waitFor(() => expect(container.querySelector(".notice.error")?.textContent).toContain("disco lleno"));
    expect(texts(container)).toEqual(["A"]);
  });

  it("no hay nada para exportar: solo el nombre y la prioridad", () => {
    const { container } = render(TasksSection, { api: api().a });
    expect(container.querySelector(".task")).toBeNull();
    expect(screen.queryByText(/Exportar/i)).toBeNull();
  });
});
