// Canned Tauri command responses for the mock-IPC Playwright smoke.
//
// These drive the React frontend without the Rust backend. Shapes mirror what
// the real desktop commands return (see crates/repodesk-core + src-tauri/src/commands),
// but only the fields the UI reads are filled in. The mock-IPC layer returns the
// matching entry for each invoke(command); unknown commands resolve to `null`,
// which every shell hook tolerates (optionalCommand swallows it).

export type CommandFixtures = Record<string, unknown>;

/** A fully onboarded, commit-ready workspace — the daily-loop happy path. */
export const onboardedFixtures: CommandFixtures = {
  desktop_snapshot: {
    project: { name: "RepoDesk" },
    task: { title: "Wire N2 E2E smoke" },
  },
  get_active_project_config: {
    name: "RepoDesk",
    project_type: "rust",
    main_language: "rust",
    ignore_rules: [],
  },
  db_status: { ok: true, path: "/tmp/repodesk-dev/repodesk.db" },

  project_list_configs: [
    { name: "RepoDesk", path: "/Users/you/code/repodesk", project_type: "rust" },
    { name: "my-api", path: "/Users/you/code/my-api", project_type: "node" },
  ],
  git_file_diff:
    "diff --git a/src/app.ts b/src/app.ts\n@@ -1,3 +1,4 @@\n context\n-old line\n+new line\n+added line\n",

  task_list: [
    {
      config: {
        id: "task-n2-e2e",
        project_name: "RepoDesk",
        title: "Wire N2 E2E smoke",
        status: "open",
        run_dir: "/tmp/repodesk-dev/RepoDesk/task-n2-e2e",
        created_at: "2026-06-16T10:00:00Z",
        updated_at: "2026-06-16T10:00:00Z",
      },
      is_active: true,
    },
  ],

  git_workspace_snapshot: {
    branch: "feat/n2-e2e",
    is_dirty: true,
    unstaged: [
      "apps/desktop/e2e/daily-loop.spec.ts",
      "apps/desktop/package.json",
      ".github/workflows/ci.yml",
    ],
    changed_files: [
      { path: "apps/desktop/e2e/daily-loop.spec.ts", status_code: "A" },
      { path: "apps/desktop/package.json", status_code: "M" },
      { path: ".github/workflows/ci.yml", status_code: "M" },
    ],
  },

  model_health_snapshot: {
    providers: [
      { name: "ollama", reachability: "working", models: [{ name: "llama3" }] },
      { name: "openai", reachability: "unconfigured", models: [] },
    ],
  },

  token_usage_snapshot: {
    totals: {
      entries_count: 4,
      total_input_tokens: 9000,
      total_output_tokens: 3840,
      total_tokens: 12840,
      today_total_tokens: 4000,
      remaining_daily_tokens: 96000,
    },
    by_provider: [{ provider: "ollama", model: null, input_tokens: 9000, output_tokens: 3840, total_tokens: 12840, estimated_cost_units: 0, currency_label: "USD" }],
    by_model: [],
    active_artifacts: [
      { kind: "context", title: "Context pack", path: "context.md", exists: true, estimated_tokens: 1800, status: "ok", recommendation: "" },
    ],
    cost_estimate: { estimated_total_units: 0, currency_label: "USD", note: "local estimate" },
  },

  product_workflow_state: {
    project_ok: true,
    task_ok: true,
    primary_cta: "Run safety checks",
    recommended_action_id: "smart-context-build",
    steps: [
      { id: "context", title: "Build bounded context", status: "done", description: "Context assembled within budget." },
      { id: "checks", title: "Run safety checks", status: "pending", description: "Safety + budget gates before AI." },
      { id: "commit", title: "Commit when safe", status: "pending", description: "Pre-commit secret scan + readiness." },
    ],
    commit_readiness: {
      status: "ready",
      headline: "All checks passed — safe to commit",
      branch: "feat/n2-e2e",
      changed_count: 3,
      blockers: [],
      warnings: [],
    },
  },

  desktop_actions: [
    { id: "smart-context-build", label: "Build bounded context", description: "Assemble safe, bounded context.", risk: "safe", category: "Context" },
    { id: "checks-run", label: "Run checks", description: "Run safety + budget gates.", risk: "safe", category: "Checks" },
  ],
  action_history: [],

  orchestration_runs: [
    {
      run_id: "run-20260616-101500",
      goal: "Wire N2 E2E smoke",
      status: "completed",
      dry_run: false,
      started_at: "2026-06-16T10:15:00Z",
      finished_at: "2026-06-16T10:16:30Z",
      step_count: 3,
      total_cost_units: 0,
    },
  ],
  task_timeline: [
    {
      timestamp: "2026-06-16T10:16:30Z",
      project: "RepoDesk",
      task_id: "task-n2-e2e",
      module_name: "orchestrator",
      level: "info",
      message: "orchestration run-20260616-101500 finished: Completed",
      metadata: { run_id: "run-20260616-101500" },
    },
  ],

  routing_snapshot: {
    request: { task_kind: "edit", changed_file_count: 3 },
    decision: {
      recommended_provider: "ollama",
      recommended_model: "llama3",
      decision_level: "ok",
      task_kind: "edit",
      score: 87,
      estimated_total_tokens: 12840,
      fallback_provider: "manual",
      blockers: [],
      warnings: [],
      required_guardrails: ["secret-scan"],
      candidates: [
        { provider: "ollama", model: "llama3", kind: "local", score: 87, blocked: false, estimated_cost_units: 0 },
      ],
    },
  },
};

/** A brand-new REPODESK_HOME — no project, no task. Forces onboarding. */
export const firstRunFixtures: CommandFixtures = {
  desktop_snapshot: {},
  get_active_project_config: null,
  db_status: { ok: true, path: "/tmp/repodesk-firstrun/repodesk.db" },
  git_workspace_snapshot: { branch: "-", is_dirty: false, changed_files: [] },
  model_health_snapshot: { providers: [] },
  token_usage_snapshot: { totals: { total_tokens: 0 }, cost_estimate: { currency_label: "USD" } },
  product_workflow_state: {
    project_ok: false,
    task_ok: false,
    steps: [],
    commit_readiness: { status: "" },
  },
  desktop_actions: [],
  action_history: [],
  routing_snapshot: null,
};
