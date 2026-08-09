import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  TASK_RUNNER_KEY,
  runAllProjectTasks,
  runProjectTask,
  taskRunnerSnapshot,
  type ProjectTask,
  type ProjectTaskKind,
  type TaskRunResult,
  type TaskRunStatus,
} from "../shared/api/taskRunner";
import { useWorkspace } from "../shared/hooks/useWorkspace";
import { errorToMessage } from "../shared/utils/helpers";

const KIND_LABEL: Record<ProjectTaskKind, string> = {
  format: "Format",
  lint: "Lint",
  typecheck: "Types",
  test: "Test",
  security: "Security",
  check: "Check",
};

function statusLabel(status: TaskRunStatus): string {
  if (status === "timeout") return "Timed out";
  return status.charAt(0).toUpperCase() + status.slice(1);
}

function durationLabel(durationMs: number): string {
  if (durationMs < 1_000) return `${durationMs}ms`;
  return `${(durationMs / 1_000).toFixed(durationMs < 10_000 ? 1 : 0)}s`;
}

function resultTone(status: TaskRunStatus): string {
  if (status === "passed") return "ok";
  if (status === "blocked") return "neutral";
  return "danger";
}

function TaskRow({
  task,
  result,
  busy,
  onRun,
  onInspect,
}: {
  task: ProjectTask;
  result: TaskRunResult | null;
  busy: boolean;
  onRun: () => void;
  onInspect: () => void;
}) {
  return (
    <div className={`task-runner-row${task.runnable ? "" : " blocked"}`}>
      <span className={`task-kind kind-${task.kind}`}>{KIND_LABEL[task.kind]}</span>
      <button type="button" className="task-runner-main" onClick={result ? onInspect : undefined} disabled={!result}>
        <strong>{task.label}</strong>
        <code>{task.command}</code>
      </button>
      <div className="task-runner-state">
        {result ? (
          <button type="button" className={`task-result ${resultTone(result.status)}`} onClick={onInspect}>
            <span>{statusLabel(result.status)}</span>
            <small>{durationLabel(result.duration_ms)}</small>
          </button>
        ) : task.validation_error ? (
          <span className="task-blocked-label" title={task.validation_error}>Blocked</span>
        ) : (
          <span className="task-not-run">Not run</span>
        )}
        <button
          type="button"
          className="tiny-button task-run-button"
          disabled={busy || !task.runnable}
          onClick={onRun}
        >
          Run
        </button>
      </div>
    </div>
  );
}

function TaskRunDetail({ result }: { result: TaskRunResult }) {
  const output = [
    result.stderr.trim() ? `STDERR\n${result.stderr.trim()}` : "",
    result.stdout.trim() ? `STDOUT\n${result.stdout.trim()}` : "",
  ].filter(Boolean).join("\n\n");

  return (
    <section className="task-run-detail" aria-label={`Last run for ${result.label}`}>
      <header>
        <div>
          <strong>{result.label}</strong>
          <code>{result.command}</code>
        </div>
        <div className="task-run-detail-meta">
          <span className={`pill ${resultTone(result.status)}`}>{statusLabel(result.status)}</span>
          <span>{durationLabel(result.duration_ms)}</span>
          {result.exit_code !== null ? <span>exit {result.exit_code}</span> : null}
        </div>
      </header>
      {output ? (
        <pre>{output}</pre>
      ) : (
        <div className="task-run-clean-output">No command output.</div>
      )}
      {(result.stdout_truncated || result.stderr_truncated) ? (
        <small className="task-output-note">Output is bounded to the most recent 64 KiB per stream.</small>
      ) : null}
    </section>
  );
}

export function TaskRunnerPanel({ onOpenProblems }: { onOpenProblems: () => void }) {
  const { hasProject, projectName } = useWorkspace();
  const queryClient = useQueryClient();
  const [results, setResults] = useState<Record<string, TaskRunResult>>({});
  const [selectedTaskId, setSelectedTaskId] = useState<string | null>(null);

  const snapshot = useQuery({
    queryKey: [...TASK_RUNNER_KEY, projectName ?? "none"],
    queryFn: taskRunnerSnapshot,
    enabled: hasProject,
    staleTime: 15_000,
    refetchOnWindowFocus: true,
  });

  const refreshWorkspaceAfterRun = () => {
    void queryClient.invalidateQueries({ queryKey: ["git"] });
    void queryClient.invalidateQueries({ queryKey: ["code"] });
    void queryClient.invalidateQueries({ queryKey: ["work"] });
  };

  const runOne = useMutation({
    mutationFn: runProjectTask,
    onSuccess: (result) => {
      setResults((current) => ({ ...current, [result.task_id]: result }));
      setSelectedTaskId(result.task_id);
      refreshWorkspaceAfterRun();
    },
  });

  const runAll = useMutation({
    mutationFn: runAllProjectTasks,
    onSuccess: (batch) => {
      const next: Record<string, TaskRunResult> = {};
      for (const result of batch.results) next[result.task_id] = result;
      setResults(next);
      const firstFailure = batch.results.find((result) => result.status !== "passed");
      setSelectedTaskId(firstFailure?.task_id ?? batch.results.at(-1)?.task_id ?? null);
      refreshWorkspaceAfterRun();
    },
  });

  const busy = runOne.isPending || runAll.isPending;
  const error = runOne.error ?? runAll.error ?? snapshot.error;
  const tasks = snapshot.data?.tasks ?? [];
  const runnableCount = tasks.filter((task) => task.runnable).length;
  const selected = selectedTaskId ? results[selectedTaskId] ?? null : null;
  const summary = useMemo(() => {
    const values = Object.values(results);
    return {
      passed: values.filter((result) => result.status === "passed").length,
      failed: values.filter((result) => result.status !== "passed").length,
    };
  }, [results]);

  if (!hasProject) {
    return <div className="task-runner-empty"><strong>No project selected.</strong><span>Connect a project to discover its configured checks.</span></div>;
  }

  if (snapshot.isLoading) {
    return <div className="task-runner-empty"><strong>Loading tasks…</strong></div>;
  }

  return (
    <div className="task-runner-panel">
      <div className="task-runner-toolbar">
        <div className="task-runner-heading">
          <strong>Project tasks</strong>
          <span>{snapshot.data?.project ?? projectName}</span>
          <small>{tasks.length} configured</small>
          {snapshot.data?.truncated ? <small className="warn">list capped</small> : null}
        </div>
        <div className="task-runner-actions">
          {summary.failed > 0 ? <button type="button" className="tiny-button" onClick={onOpenProblems}>Problems {summary.failed}</button> : null}
          <button type="button" className="tiny-button" disabled={busy} onClick={() => void snapshot.refetch()}>Refresh</button>
          <button
            type="button"
            className="primary-button compact"
            disabled={busy || runnableCount === 0}
            onClick={() => runAll.mutate()}
          >
            {runAll.isPending ? "Running…" : "Run all"}
          </button>
        </div>
      </div>

      {error ? <div className="task-runner-error">{errorToMessage(error)}</div> : null}

      {tasks.length === 0 ? (
        <div className="task-runner-empty">
          <strong>No project tasks configured.</strong>
          <span>RepoDesk exposes only allowlisted checks from this project's configuration.</span>
        </div>
      ) : (
        <div className="task-runner-list">
          {tasks.map((task) => (
            <TaskRow
              key={task.id}
              task={task}
              result={results[task.id] ?? null}
              busy={busy}
              onRun={() => runOne.mutate(task.id)}
              onInspect={() => setSelectedTaskId(task.id)}
            />
          ))}
        </div>
      )}

      {selected ? <TaskRunDetail result={selected} /> : null}
    </div>
  );
}
