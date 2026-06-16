import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { TaskSummary, taskList, taskNew, taskUse } from "../../shared/api/workflow";

/**
 * Multi-task control for the active project: shows every task (newest first),
 * lets you switch the active one, and create a new task inline. Switching a task
 * re-scopes everything downstream (context, routing, RepoPilot trend), so a
 * successful switch/create invalidates all queries.
 */
export function TaskSwitcher() {
  const queryClient = useQueryClient();
  const [title, setTitle] = useState("");
  const [error, setError] = useState("");

  const { data: tasks = [], isLoading } = useQuery<TaskSummary[]>({
    queryKey: ["task_list"],
    queryFn: taskList,
  });

  const refetchAll = () => queryClient.invalidateQueries();

  const switchTask = useMutation({
    mutationFn: (id: string) => taskUse(id),
    onSuccess: (result) => {
      if (!result.ok) {
        setError(result.stderr || "Could not switch task.");
        return;
      }
      setError("");
      void refetchAll();
    },
    onError: (e: any) => setError(e?.message || String(e)),
  });

  const createTask = useMutation({
    mutationFn: (t: string) => taskNew(t),
    onSuccess: (result) => {
      if (!result.ok) {
        setError(result.stderr || "Could not create task.");
        return;
      }
      setError("");
      setTitle("");
      void refetchAll();
    },
    onError: (e: any) => setError(e?.message || String(e)),
  });

  const busy = switchTask.isPending || createTask.isPending;

  return (
    <section className="panel task-switcher wide-panel">
      <div className="panel-title-row">
        <div>
          <p className="eyebrow">Tasks</p>
          <h2>{tasks.length === 1 ? "1 task" : `${tasks.length} tasks`} in this project</h2>
        </div>
        <span className="pill neutral">one active at a time</span>
      </div>

      {isLoading ? (
        <p className="muted">Loading tasks…</p>
      ) : (
        <div className="task-list scroll-area small">
          {tasks.map((task) => {
            const { id, title: taskTitle, status } = task.config;
            return (
              <button
                key={id}
                className={`file-row ${task.is_active ? "active" : ""}`}
                disabled={busy || task.is_active}
                onClick={() => switchTask.mutate(id)}
                title={id}
              >
                <span className="task-row-main">
                  <strong>{taskTitle || id}</strong>
                  <small>{id}</small>
                </span>
                <span className="file-badges">
                  {status === "closed" && <span className="pill neutral">closed</span>}
                  {task.is_active && <span className="pill ok">active</span>}
                </span>
              </button>
            );
          })}
        </div>
      )}

      <div className="commit-box" style={{ marginTop: 10 }}>
        <input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="New task title"
          disabled={busy}
          onKeyDown={(e) => {
            if (e.key === "Enter" && title.trim() && !busy) createTask.mutate(title.trim());
          }}
        />
        <button
          className="ghost-button"
          disabled={!title.trim() || busy}
          onClick={() => createTask.mutate(title.trim())}
        >
          {createTask.isPending ? "Creating…" : "Create task"}
        </button>
      </div>

      {error && <div className="notice danger">{error}</div>}
    </section>
  );
}
