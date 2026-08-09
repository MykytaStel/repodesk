import { callCommand } from "./queries";

export const TASK_RUNNER_KEY = ["task-runner", "snapshot"] as const;

export type ProjectTaskKind = "format" | "lint" | "typecheck" | "test" | "security" | "check";
export type TaskRunStatus = "passed" | "failed" | "timeout" | "blocked";

export type ProjectTask = {
  id: string;
  label: string;
  command: string;
  kind: ProjectTaskKind;
  runnable: boolean;
  validation_error: string | null;
};

export type TaskRunnerSnapshot = {
  project: string;
  tasks: ProjectTask[];
  truncated: boolean;
};

export type TaskRunResult = {
  project: string;
  task_id: string;
  label: string;
  kind: ProjectTaskKind;
  command: string;
  status: TaskRunStatus;
  exit_code: number | null;
  duration_ms: number;
  started_at: string;
  finished_at: string;
  stdout: string;
  stderr: string;
  stdout_truncated: boolean;
  stderr_truncated: boolean;
};

export type TaskRunBatch = {
  project: string;
  started_at: string;
  finished_at: string;
  success: boolean;
  passed: number;
  failed: number;
  timed_out: number;
  blocked: number;
  results: TaskRunResult[];
};

export function taskRunnerSnapshot(): Promise<TaskRunnerSnapshot> {
  return callCommand("task_runner_snapshot");
}

export function runProjectTask(taskId: string): Promise<TaskRunResult> {
  return callCommand("task_runner_run", { taskId });
}

export function runAllProjectTasks(): Promise<TaskRunBatch> {
  return callCommand("task_runner_run_all");
}
