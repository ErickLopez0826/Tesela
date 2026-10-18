<!-- Tareas del día: solo el nombre y la prioridad. Se reordenan con las flechas y, al marcarlas como hechas, se eliminan. -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { Priority, Task, TasksApi } from "../../api/tasks";
  import { MAX_TASK_CHARS, PRIORITIES, PRIORITY_LABEL, cyclePriority, move, newTask, setPriority, sortByPriority, without } from "../../stores/tasks";

  let { api }: { api: TasksApi } = $props();

  /** Lo que tarda en desvanecerse una tarea hecha antes de quitarse. */
  const DONE_MS = 380;

  let tasks: Task[] = $state([]);
  let text = $state("");
  let priority: Priority = $state("medium");
  let finishing: string[] = $state([]);
  let error: string | null = $state(null);

  // Los guardados van uno detrás de otro, en el orden en que se pidieron.
  let queue: Promise<unknown> = Promise.resolve();

  function persist(next: Task[]) {
    const before = tasks;
    tasks = next;
    error = null;
    queue = queue
      .then(() => api.save(next))
      .catch((e) => {
        error = String((e as { message?: string })?.message ?? e);
        tasks = before;
      });
  }

  onMount(() => {
    api.list().then((list) => (tasks = list)).catch((e) => (error = String(e)));
  });

  // Si el panel cambia de pestaña mientras una tarea se desvanece, ya queda quitada.
  onDestroy(() => {
    if (finishing.length) persist(tasks.filter((t) => !finishing.includes(t.id)));
  });

  function add() {
    const task = newTask(text, priority);
    if (!task) return;
    persist([...tasks, task]);
    text = "";
  }

  function done(task: Task) {
    if (finishing.includes(task.id)) return;
    finishing = [...finishing, task.id];
    setTimeout(() => {
      if (!finishing.includes(task.id)) return;
      finishing = finishing.filter((id) => id !== task.id);
      persist(without(tasks, task.id));
    }, DONE_MS);
  }

  const reorder = (task: Task, delta: -1 | 1) => {
    const next = move(tasks, task.id, delta);
    if (next !== tasks) persist(next);
  };

  const sorted = $derived(sortByPriority(tasks));
  const isSorted = $derived(sorted.every((t, i) => t.id === tasks[i].id));
</script>

<section class="section tasks">
  <h3>Tareas <span class="count">{tasks.length}</span></h3>

  <form
    class="add"
    onsubmit={(e) => {
      e.preventDefault();
      add();
    }}
  >
    <input bind:value={text} maxlength={MAX_TASK_CHARS} placeholder="Nueva tarea…" aria-label="Nueva tarea" />
    <select bind:value={priority} aria-label="Prioridad de la nueva tarea">
      {#each PRIORITIES as p (p)}<option value={p}>{PRIORITY_LABEL[p]}</option>{/each}
    </select>
    <button class="btn primary" type="submit" disabled={!text.trim()}>Agregar</button>
  </form>

  <ul class="list">
    {#each tasks as t, i (t.id)}
      <li class="task" class:done={finishing.includes(t.id)} data-priority={t.priority}>
       <div class="card">
        <button class="check" aria-label={`Marcar como hecha: ${t.text}`} onclick={() => done(t)}>
          <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 8.5l3.2 3.2L13 4.8" /></svg>
        </button>
        <span class="text">{t.text}</span>
        <button class="prio" aria-label={`Prioridad de ${t.text}: ${PRIORITY_LABEL[t.priority]}. Cambiar`} onclick={() => persist(setPriority(tasks, t.id, cyclePriority(t.priority)))}>
          {PRIORITY_LABEL[t.priority]}
        </button>
        <span class="move">
          <button aria-label={`Subir: ${t.text}`} disabled={i === 0} onclick={() => reorder(t, -1)}>↑</button>
          <button aria-label={`Bajar: ${t.text}`} disabled={i === tasks.length - 1} onclick={() => reorder(t, 1)}>↓</button>
        </span>
       </div>
      </li>
    {:else}
      <li class="empty hint">Sin tareas pendientes.</li>
    {/each}
  </ul>

  {#if tasks.length > 1}
    <div class="footer">
      <button class="btn mini" disabled={isSorted} onclick={() => persist(sorted)}>Ordenar por prioridad</button>
    </div>
  {/if}
  {#if error}<div class="notice error">{error}</div>{/if}
</section>

<style>
  .tasks { display: flex; flex-direction: column; gap: 8px; min-width: 0; }
  h3 { display: flex; }
  .count { margin-left: auto; font-weight: 400; letter-spacing: 0.06em; text-transform: none; }
  .add { display: flex; gap: 6px; align-items: center; }
  .add input { flex: 1; min-width: 0; font: 13px "Segoe UI", system-ui, sans-serif; color: var(--fg); background: rgba(0, 0, 0, 0.5); border: 1px solid var(--line); border-radius: 12px; padding: 7px 10px; }
  .add input:focus, .add select:focus { outline: none; border-color: var(--accent); }
  .add select { font: 11px var(--mono); color: var(--fg); background: rgba(0, 0, 0, 0.5); border: 1px solid var(--line); border-radius: 12px; padding: 6px 6px; cursor: pointer; }
  .add select option { background: #121214; }
  .list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; max-height: 190px; overflow: auto; scrollbar-width: thin; scrollbar-color: rgba(255, 255, 255, 0.2) transparent; }
  /* Al marcarla como hecha la tarea se desvanece, se desliza y colapsa su altura: las de abajo suben suavemente. */
  .task { display: grid; grid-template-rows: 1fr; margin-bottom: 5px; transition: grid-template-rows 0.38s cubic-bezier(.3, 0, .2, 1), margin-bottom 0.38s cubic-bezier(.3, 0, .2, 1), opacity 0.3s ease, transform 0.38s ease; }
  .card { display: flex; align-items: center; gap: 8px; min-height: 0; overflow: hidden; padding: 6px 8px; border-radius: 12px; background: rgba(255, 255, 255, 0.05); border: 1px solid var(--line); border-left: 3px solid var(--prio, var(--muted)); }
  .task[data-priority="high"] { --prio: #ff5a5f; }
  .task[data-priority="medium"] { --prio: #ffc062; }
  .task[data-priority="low"] { --prio: var(--muted); }
  .task.done { grid-template-rows: 0fr; margin-bottom: 0; opacity: 0; transform: translateX(14px); }
  .task.done .card { padding-top: 0; padding-bottom: 0; border-color: transparent; }
  .task.done .text { text-decoration: line-through; }
  .check { display: grid; place-items: center; flex: none; width: 22px; height: 22px; padding: 0; border-radius: 50%; border: 1.5px solid var(--muted); background: transparent; color: transparent; cursor: pointer; transition: background 0.25s, color 0.25s, border-color 0.25s; }
  .check:hover, .task.done .check { background: var(--accent); border-color: var(--accent); color: var(--on-accent); }
  .text { flex: 1; min-width: 0; overflow-wrap: anywhere; font-size: 13px; }
  .prio { flex: none; padding: 2px 9px; border-radius: 999px; border: 1px solid var(--prio, var(--line)); background: transparent; color: var(--prio, var(--fg)); font: 10px var(--mono); letter-spacing: 0.06em; text-transform: uppercase; cursor: pointer; }
  .move { display: flex; flex: none; gap: 2px; }
  .move button { width: 22px; height: 22px; padding: 0; border-radius: 8px; border: 1px solid var(--line); background: transparent; color: var(--fg); cursor: pointer; font-size: 12px; line-height: 1; }
  .move button:disabled { opacity: 0.25; cursor: default; }
  .empty { padding: 10px 4px; text-align: center; background: transparent; border: none; }
  .footer { display: flex; justify-content: flex-end; }
  .mini { padding: 3px 10px; font-size: 10px; }
  @media (prefers-reduced-motion: reduce) { .task { transition: none; } }
</style>
