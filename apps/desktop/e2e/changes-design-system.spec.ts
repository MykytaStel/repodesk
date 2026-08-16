import { expect, test, type Page } from "@playwright/test";
import { currentOnboardedFixtures } from "./current-fixtures";
import type { CommandFixtures } from "./fixtures";
import { installMockIpc } from "./mock-ipc";

function tabButton(page: Page, name: string) {
  return page.getByRole("button", { name: new RegExp(`^${name} —`) }).first();
}

function engineeringSnapshot(options?: {
  attribution?: "exact_isolated" | "manual" | "legacy_unknown" | "unattributed";
  exactRequired?: boolean;
  gateState?:
    | "no_change_set"
    | "scope_violation"
    | "needs_review"
    | "rejected"
    | "verification_required"
    | "verification_running"
    | "verification_failed"
    | "verification_stale"
    | "ready"
    | "committed";
  blockers?: string[];
  verificationState?: "passed" | "failed" | "running" | "not_run";
  verificationFresh?: boolean | null;
  staleReason?: string | null;
  scopeViolation?: boolean;
}) {
  const attribution = options?.attribution ?? "exact_isolated";
  const gateState = options?.gateState ?? "ready";
  const verificationState = options?.verificationState ?? "passed";
  const verificationFresh = options?.verificationFresh ?? true;
  const blockers = options?.blockers ?? [];
  const scopeStatus = options?.scopeViolation ? "violation" : "compliant";
  const fileScopeState = options?.scopeViolation ? "out_of_scope" : "allowed";
  const base = currentOnboardedFixtures.work_engineering_intelligence as Record<string, unknown>;

  return {
    ...base,
    change_governance: {
      work_item_id: "task-n2-e2e",
      changeset_id: "run-semantic-changeset",
      files: [{ path: "src/app.ts", scope_state: fileScopeState }],
      origin: {
        execution_id: attribution === "manual" ? null : "run-semantic",
        workers: [{ kind: attribution === "manual" ? "human" : "coding_agent", id: attribution === "manual" ? "human" : "codex_cli", provider: null, model: null }],
        execution_mode: attribution === "manual" ? "manual_handoff" : "agent_run",
      },
      scope_status: scopeStatus,
      scope_override: null,
      review_state: "accepted",
      verification: {
        state: verificationState,
        verification_id: verificationState === "not_run" ? null : "verify-semantic",
        command_count: verificationState === "not_run" ? 0 : 1,
        evidence: [],
        error: verificationState === "failed" ? "Verification command failed" : null,
        fresh: verificationFresh,
        stale_reason: options?.staleReason ?? null,
      },
      committed: false,
      commit_sha: null,
      gate: { state: gateState, ready: gateState === "ready", blockers, warnings: [] },
    },
    changeset_passport: {
      work_item_id: "task-n2-e2e",
      changeset_id: "run-semantic-changeset",
      run_id: "run-semantic",
      baseline_commit: "1111111111111111111111111111111111111111",
      attribution: {
        strength: attribution,
        workspace_id: attribution === "exact_isolated" ? "wt-run-semantic-implement" : null,
        baseline_commit: "1111111111111111111111111111111111111111",
        reason: attribution === "manual" ? "change entered through an explicit manual handoff" : null,
      },
      changed_file_count: 1,
      scope_status: scopeStatus,
      review_state: "accepted",
      verification_state: verificationState,
      verification_fresh: verificationFresh === true,
      acceptance: { configured: false, total: 0, proven: 0, failed: 0, unproven: 0 },
      committed: false,
      commit_sha: null,
      gate: { state: gateState, ready: gateState === "ready", blockers, warnings: [] },
    },
    safe_commit_manifest: {
      version: 2,
      work_item_id: "task-n2-e2e",
      run_id: "run-semantic",
      changeset_digest: "changeset-digest",
      parent_head_sha: "1111111111111111111111111111111111111111",
      current_head_sha: "2222222222222222222222222222222222222222",
      reviewed_tree_sha: "3333333333333333333333333333333333333333",
      verification_tree_sha: "3333333333333333333333333333333333333333",
      verification_verified_at: "2026-08-16T12:00:00Z",
      verification_commands: [{ command: "pnpm test", success: true }],
      reviewed_paths: ["src/app.ts"],
      staged_paths: ["src/app.ts"],
      scope: {
        status: scopeStatus,
        allowed: !options?.scopeViolation,
        overridden: false,
        override_event_id: null,
        out_of_scope_files: options?.scopeViolation ? ["src/app.ts"] : [],
        protected_files: [],
      },
      acceptance: {
        configured: false,
        work_item_id: "task-n2-e2e",
        current_run_id: "run-semantic",
        criteria: [],
        proven: 0,
        failed: 0,
        unproven: 0,
      },
      attribution: {
        strength: attribution,
        workspace_id: attribution === "exact_isolated" ? "wt-run-semantic-implement" : null,
        baseline_commit: "1111111111111111111111111111111111111111",
        reason: attribution === "manual" ? "change entered through an explicit manual handoff" : null,
      },
      exact_attribution_required: options?.exactRequired ?? false,
      commit_sha: null,
      state: blockers.length === 0 && gateState === "ready" ? "ready" : "blocked",
      ready: blockers.length === 0 && gateState === "ready",
      blockers,
      warnings: [],
      manifest_digest: "abcdef1234567890abcdef1234567890",
    },
  };
}

function fixturesWithEngineering(value: unknown): CommandFixtures {
  return {
    ...currentOnboardedFixtures,
    work_engineering_intelligence: value,
  };
}

async function openManifest(page: Page) {
  await tabButton(page, "Changes").click();
  await page.getByRole("button", { name: /Manifest/ }).click();
}

test.describe("Changes semantic design-system reference", () => {
  test("exact isolated attribution is an explicit positive semantic state", async ({ page }) => {
    await installMockIpc(page, fixturesWithEngineering(engineeringSnapshot()));
    await page.goto("/");
    await openManifest(page);

    const exact = page.getByText("Exact · isolated worktree", { exact: true });
    await expect(exact).toBeVisible();
    await expect(exact.locator("xpath=ancestor-or-self::*[@data-semantic-tone][1]")).toHaveAttribute(
      "data-semantic-tone",
      "positive",
    );
    await expect(page.getByText("Ready to commit", { exact: true }).last()).toBeVisible();
  });

  test("manual attribution stays neutral while exact-attribution policy blocker is critical", async ({ page }) => {
    const snapshot = engineeringSnapshot({
      attribution: "manual",
      exactRequired: true,
      blockers: ["Commit blocked: this Project requires exact producer attribution."],
    });
    await installMockIpc(page, fixturesWithEngineering(snapshot));
    await page.goto("/");
    await openManifest(page);

    const manual = page.getByText("Manual handoff", { exact: true });
    await expect(manual).toBeVisible();
    await expect(manual.locator("xpath=ancestor-or-self::*[@data-semantic-tone][1]")).toHaveAttribute(
      "data-semantic-tone",
      "neutral",
    );
    const blocker = page.getByRole("alert").filter({ hasText: "requires exact producer attribution" });
    await expect(blocker).toBeVisible();
    await expect(blocker).toHaveAttribute("data-semantic-tone", "critical");
  });

  test("stale verification is attention evidence and keeps the stale reason", async ({ page }) => {
    const snapshot = engineeringSnapshot({
      verificationState: "passed",
      verificationFresh: false,
      staleReason: "The reviewed tree changed after verification.",
      gateState: "verification_stale",
      blockers: ["Verification receipt is stale."],
    });
    await installMockIpc(page, fixturesWithEngineering(snapshot));
    await page.goto("/");
    await openManifest(page);

    const stale = page.getByText("Passed · stale", { exact: true });
    await expect(stale).toBeVisible();
    await expect(stale.locator("xpath=ancestor-or-self::*[@data-semantic-tone][1]")).toHaveAttribute(
      "data-semantic-tone",
      "attention",
    );
    await expect(page.getByText("The reviewed tree changed after verification.", { exact: true })).toBeVisible();
  });

  test("scope violation is critical and exposes one override action", async ({ page }) => {
    const snapshot = engineeringSnapshot({
      gateState: "scope_violation",
      scopeViolation: true,
      blockers: ["One or more changed paths are outside the Work Item scope."],
    });
    await installMockIpc(page, fixturesWithEngineering(snapshot));
    await page.goto("/");
    await openManifest(page);

    await expect(page.getByRole("alert").filter({ hasText: "outside the Work Item scope" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Record one-time override" })).toHaveCount(1);
  });

  test("governance loading uses the shared accessible loading state", async ({ page }) => {
    await installMockIpc(
      page,
      fixturesWithEngineering({ __mock_delay_ms: 1_000, __mock_value: engineeringSnapshot() }),
    );
    await page.goto("/");
    await openManifest(page);

    await expect(page.getByRole("status").filter({ hasText: "Loading ChangeSet evidence" })).toBeVisible();
  });

  test("governance failure uses the shared accessible error state", async ({ page }) => {
    await installMockIpc(
      page,
      fixturesWithEngineering({ __mock_error: "fixture governance failure" }),
    );
    await page.goto("/");
    await openManifest(page);

    await expect(page.getByRole("alert").filter({ hasText: "Change governance unavailable" })).toBeVisible({ timeout: 15_000 });
  });
});