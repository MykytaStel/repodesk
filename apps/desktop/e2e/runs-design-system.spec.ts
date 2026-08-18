import { test, expect, type Page } from "@playwright/test";
import { installMockIpc } from "./mock-ipc";
import { currentOnboardedFixtures } from "./current-fixtures";
import type { CommandFixtures } from "./fixtures";

function tabButton(page: Page, name: string) {
  return page.getByRole("button", { name: new RegExp(`^${name} —`) }).first();
}

async function bootRuns(page: Page, overrides: CommandFixtures = {}) {
  await installMockIpc(page, { ...currentOnboardedFixtures, ...overrides });
  await page.goto("/");
  await tabButton(page, "Runs").click();
}

function engineeringWith(overrides: Record<string, unknown>) {
  return {
    ...(currentOnboardedFixtures.work_engineering_intelligence as Record<string, unknown>),
    ...overrides,
  };
}

test.describe("Runs design-system convergence", () => {
  test("run, disposition, worker, verification and commit states use semantic tones", async ({ page }) => {
    const base = currentOnboardedFixtures.work_engineering_intelligence as Record<string, any>;
    await bootRuns(page, {
      work_engineering_intelligence: engineeringWith({
        run_evidence: {
          ...base.run_evidence,
          status: "completed",
          workers: [{
            step_id: "implement",
            agent: "Codex",
            provider: "codex_cli",
            model: "codex",
            status: "failed",
            changed_files: ["src/app.ts"],
            input_tokens: 1_200,
            output_tokens: 200,
            cost_units: 0,
          }],
          verification: {
            ...base.run_evidence.verification,
            state: "passed",
            commands: [
              { command: "cargo test", success: true },
              { command: "cargo clippy", success: false },
            ],
          },
          commit: { ...base.run_evidence.commit, committed: false },
        },
        run_observability: {
          ...base.run_observability,
          disposition: {
            state: "blocked",
            stage: "verification",
            code: "verification_failed",
            title: "Verification blocked",
            detail: "A canonical verification command failed.",
          },
        },
      }),
    });

    await expect(page.getByText("completed", { exact: true }).first()).toHaveAttribute("data-semantic-tone", "positive");
    await expect(page.getByText("Verification blocked", { exact: true }).locator("xpath=ancestor-or-self::*[@data-semantic-tone][1]")).toHaveAttribute("data-semantic-tone", "critical");
    await expect(page.getByText("failed", { exact: true }).first()).toHaveAttribute("data-semantic-tone", "critical");
    await expect(page.getByText("passed", { exact: true }).first()).toHaveAttribute("data-semantic-tone", "positive");
    await expect(page.getByText("not committed", { exact: true })).toHaveAttribute("data-semantic-tone", "neutral");
  });

  test("partial runs and stale acceptance evidence never look positive", async ({ page }) => {
    const base = currentOnboardedFixtures.work_engineering_intelligence as Record<string, any>;
    await bootRuns(page, {
      orchestration_runs: [{
        run_id: "run-20260616-101500",
        goal: "Wire N2 E2E smoke",
        status: "partial",
        dry_run: false,
        started_at: "2026-06-16T10:15:00Z",
        finished_at: "2026-06-16T10:16:30Z",
        step_count: 1,
        total_cost_units: 0,
      }],
      work_engineering_intelligence: engineeringWith({
        run_evidence: {
          ...base.run_evidence,
          status: "partial",
          acceptance: {
            configured: true,
            work_item_id: "task-n2-e2e",
            current_run_id: "run-20260616-101500",
            proven: 0,
            failed: 0,
            unproven: 1,
            criteria: [{
              criterion_id: "criterion-1",
              criterion: "Tests pass",
              status: "proven",
              command: "cargo test",
              run_id: "run-20260616-101500",
              linked_at: "2026-06-16T10:17:00Z",
              stale: true,
              stale_reason: "Verification tree changed.",
            }],
          },
          verification: { ...base.run_evidence.verification, state: "stale" },
        },
      }),
    });

    await expect(page.getByText("partial", { exact: true }).first()).toHaveAttribute("data-semantic-tone", "attention");
    await expect(page.getByText("Stale", { exact: true })).toHaveAttribute("data-semantic-tone", "attention");
    await expect(page.getByText("Verification tree changed.")).toBeVisible();
  });

  test("loading, load failure and empty history use shared accessible states", async ({ page }) => {
    await bootRuns(page, {
      orchestration_runs: { __mock_delay_ms: 700, __mock_value: [] },
    });
    await expect(page.getByRole("status").filter({ hasText: "Loading persisted runs" })).toBeVisible();

    const failedPage = await page.context().newPage();
    await bootRuns(failedPage, { orchestration_runs: { __mock_error: "fixture history failure" } });
    await expect(failedPage.getByRole("alert")).toContainText("Run history unavailable");

    const emptyPage = await page.context().newPage();
    await bootRuns(emptyPage, { orchestration_runs: [] });
    await expect(emptyPage.getByText("No persisted execution runs yet.")).toBeVisible();
  });

  test("structured History errors render their message instead of object coercion", async ({ page }) => {
    await bootRuns(page);
    await page.getByRole("button", { name: "Refresh" }).evaluate((button) => {
      const internals = (window as unknown as {
        __TAURI_INTERNALS__: { invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> };
      }).__TAURI_INTERNALS__;
      const invoke = internals.invoke.bind(internals);
      internals.invoke = (cmd, args) => {
        if (cmd === "orchestration_runs") {
          return Promise.reject({
            category: "internal",
            message: "Structured history failure",
            retryable: false,
          });
        }
        return invoke(cmd, args);
      };
      (button as HTMLButtonElement).click();
    });
    const alert = page.getByRole("alert");
    await expect(alert).toContainText("Structured history failure");
    await expect(alert).not.toContainText("[object Object]");
  });

  test("Runs keeps its three owning views", async ({ page }) => {
    await bootRuns(page);
    await expect(page.getByRole("tab", { name: "Run evidence" })).toHaveAttribute("aria-selected", "true");
    await expect(page.getByRole("tab", { name: "Provider outcomes" })).toBeVisible();
    await expect(page.getByRole("tab", { name: "Raw audit" })).toBeVisible();
  });
});
