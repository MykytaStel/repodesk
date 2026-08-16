import { test, expect, type Page } from "@playwright/test";
import { installMockIpc } from "./mock-ipc";
import { currentOnboardedFixtures } from "./current-fixtures";
import type { CommandFixtures } from "./fixtures";

const RUN_FIXTURE = {
  run_id: "run-work-semantic-fixture",
  project: "RepoDesk",
  task_id: "task-n2-e2e",
  goal: "Wire N2 E2E smoke",
  status: "completed",
  dry_run: false,
  started_at: "2026-08-16T10:00:00Z",
  finished_at: "2026-08-16T10:01:00Z",
  results: [],
  total_input_tokens: 1_200,
  total_output_tokens: 300,
  total_cost_units: 0,
};

function phaseState(current: "scope" | "prepare" | "execute" | "review" | "verify" | "finish") {
  const order = ["scope", "prepare", "execute", "review", "verify", "finish"] as const;
  const index = order.indexOf(current);
  const titles = {
    scope: "Scope",
    prepare: "Prepare",
    execute: "Execute",
    review: "Review",
    verify: "Verify",
    finish: "Finish",
  } as const;
  return {
    current,
    complete: false,
    execution_mode: "agent_run",
    cta: {
      phase: current,
      label: current === "prepare"
        ? "Prepare context"
        : current === "execute"
          ? "Run agent"
          : current === "verify"
            ? "Run verification"
            : current === "finish"
              ? "Commit changes"
              : "Continue",
      action_id: current === "prepare" ? "context-build" : null,
    },
    phases: order.map((phase, phaseIndex) => ({
      phase,
      status: phaseIndex < index ? "done" : phaseIndex === index ? "in_progress" : "locked",
      title: titles[phase],
      summary: `${titles[phase]} fixture state`,
    })),
  };
}

async function boot(page: Page, overrides: CommandFixtures = {}) {
  await installMockIpc(page, {
    ...currentOnboardedFixtures,
    orchestrate_status: RUN_FIXTURE,
    ...overrides,
  });
  await page.goto("/");
}

function phaseChip(page: Page, title: string) {
  return page.locator(".phase-chip").filter({ hasText: title }).first();
}

function semanticAncestor(locator: ReturnType<Page["getByText"]>) {
  return locator.first().locator("xpath=ancestor-or-self::*[@data-semantic-tone][1]");
}

test.describe("Work design-system convergence", () => {
  test("phase rail, execution packet, and launch approval expose typed semantic state", async ({ page }) => {
    await boot(page);

    await expect(phaseChip(page, "Scope")).toHaveAttribute("data-semantic-tone", "positive");
    await expect(phaseChip(page, "Execute")).toHaveAttribute("data-semantic-tone", "info");
    await expect(phaseChip(page, "Review")).toHaveAttribute("data-semantic-tone", "neutral");

    const packet = page.getByRole("region", { name: "Execution packet preview" });
    await expect(packet).toBeVisible();
    await expect(packet.getByText("Prepared", { exact: true })).toHaveAttribute("data-semantic-tone", "positive");
    await expect(page.getByText("Action required", { exact: true })).toHaveAttribute("data-semantic-tone", "attention");
  });

  test("execution packet rebuild requirement remains an attention state", async ({ page }) => {
    const preview = currentOnboardedFixtures.work_strategy_execution_preview as Record<string, unknown>;
    const execution = preview.execution as Record<string, unknown>;
    const context = execution.context as Record<string, unknown>;
    await boot(page, {
      work_strategy_execution_preview: {
        ...preview,
        execution: {
          ...execution,
          context: {
            ...context,
            prepared: false,
            warning: "Prepared context is stale and must be rebuilt.",
          },
        },
      },
    });

    await expect(page.getByText("Rebuild required", { exact: true })).toHaveAttribute("data-semantic-tone", "attention");
    await expect(page.getByText("Prepared context is stale and must be rebuilt.")).toBeVisible();
  });

  test("Work authority loading and failure use the shared accessible state vocabulary", async ({ page }) => {
    await boot(page, {
      work_phase_state: {
        __mock_delay_ms: 700,
        __mock_value: phaseState("execute"),
      },
    });
    await expect(page.getByRole("status").filter({ hasText: "Loading Work Item flow" })).toBeVisible();

    const errorPage = await page.context().newPage();
    await boot(errorPage, { work_phase_state: { __mock_error: "fixture phase authority failed" } });
    const error = errorPage.getByRole("alert").filter({ hasText: "RepoDesk stopped instead of guessing" });
    await expect(error).toHaveAttribute("data-semantic-tone", "critical");
    await expect(error.getByText("fixture phase authority failed")).toBeVisible();
    await expect(error.getByRole("button", { name: "Retry" })).toBeVisible();
    await expect(error.getByRole("button", { name: "Open Runs" })).toBeVisible();
  });

  test("incomplete review evidence is critical and remains fail-closed", async ({ page }) => {
    await boot(page, {
      work_phase_state: phaseState("review"),
      orchestrate_evidence_state: {
        run_id: RUN_FIXTURE.run_id,
        status: "incomplete",
        recoverable: false,
        detail: "fixture tracked-path capture unavailable",
      },
      orchestrate_run_diffs: [
        {
          task_id: "implement",
          provider: "codex_cli",
          model: "codex",
          changed_files: ["src/app.ts"],
          diff: "diff --git a/src/app.ts b/src/app.ts\n",
          exists: true,
          truncated: false,
          warnings: [],
        },
      ],
    });

    const evidence = semanticAncestor(page.getByText(/Change evidence unavailable/));
    await expect(evidence).toHaveAttribute("data-semantic-tone", "critical");
    await expect(page.getByText(/Rerun execution to capture a trustworthy changeset/)).toBeVisible();
    await expect(page.locator(".review-file")).toHaveCount(0);
  });

  test("recovery-required review evidence says repair without rerunning", async ({ page }) => {
    await boot(page, {
      work_phase_state: phaseState("review"),
      orchestrate_evidence_state: {
        run_id: RUN_FIXTURE.run_id,
        status: "recovery_required",
        recoverable: true,
        detail: "fixture receipt persistence fault",
      },
    });

    const recovery = semanticAncestor(page.getByText(/Execution finished, but the persisted receipt needs repair/));
    await expect(recovery).toHaveAttribute("data-semantic-tone", "attention");
    await expect(page.getByText(/do not rerun the agent/i)).toBeVisible();
  });

  test("Review and Finish each expose one phase-owned primary action", async ({ page }) => {
    await boot(page, {
      work_phase_state: phaseState("review"),
      orchestrate_evidence_state: {
        run_id: RUN_FIXTURE.run_id,
        status: "ready",
        recoverable: false,
        detail: null,
      },
      orchestrate_run_diffs: [],
    });

    const reviewActions = page.locator(".work-focus-card .semantic-action-bar").filter({
      has: page.getByRole("button", { name: /Accept & stage/ }),
    });
    await expect(reviewActions.locator(".semantic-action-bar__primary")).toHaveCount(1);
    await expect(reviewActions.locator(".semantic-action-bar__primary").getByRole("button", { name: /Accept & stage/ })).toBeVisible();
    await expect(reviewActions.locator(".semantic-action-bar__destructive").getByRole("button", { name: /Reject/ })).toBeVisible();

    const finishPage = await page.context().newPage();
    await boot(finishPage, { work_phase_state: phaseState("finish") });
    const finishActions = finishPage.locator(".work-focus-card .semantic-action-bar").filter({
      has: finishPage.getByRole("button", { name: "Commit reviewed changes" }),
    });
    await expect(finishActions.locator(".semantic-action-bar__primary")).toHaveCount(1);
    await expect(finishActions.locator(".semantic-action-bar__primary").getByRole("button", { name: "Commit reviewed changes" })).toBeVisible();
  });
});