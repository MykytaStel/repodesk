import { expect, test } from "@playwright/test";
import { currentOnboardedFixtures } from "./current-fixtures";
import { installMockIpc, recordedInvocations } from "./mock-ipc";

function changedPlanPreview() {
  const first = currentOnboardedFixtures.work_strategy_execution_preview as Record<string, unknown>;
  const execution = first.execution as Record<string, unknown>;
  const context = execution.context as Record<string, unknown>;
  return {
    ...first,
    plan_fingerprint: "plan-fixture-refreshed-b5f101cc",
    execution: {
      ...execution,
      context: {
        ...context,
        context_fingerprint: "context-fixture-refreshed-7f31e95c",
        generated_at: "2026-08-12T19:00:00Z",
      },
    },
  };
}

test("capability approvals are invalidated when the exact strategy plan lock changes", async ({ page }) => {
  const firstPreview = currentOnboardedFixtures.work_strategy_execution_preview;
  const secondPreview = changedPlanPreview();
  await installMockIpc(page, {
    ...currentOnboardedFixtures,
    work_strategy_execution_preview: {
      __mock_sequence: [firstPreview, secondPreview],
    },
  });
  await page.goto("/");

  const codingApproval = page.getByRole("checkbox", { name: /Coding agent \+ isolated writes/ });
  const runButton = page.locator(".work-cta-row .primary-cta");
  await expect(codingApproval).toBeVisible();
  await codingApproval.check();
  await expect(runButton).toBeEnabled();

  // Refreshing workspace invalidates active queries. The mocked Strategy preview
  // now returns a different plan/context fingerprint under the same Auto mode.
  await page.getByRole("button", { name: "Command palette" }).click();
  await page.getByRole("textbox", { name: "Search commands" }).fill("Refresh workspace");
  await page.keyboard.press("Enter");

  await expect(codingApproval).not.toBeChecked();
  await expect(runButton).toBeDisabled();
  await expect(page.getByText(/execution packet changed after approval/i)).toBeVisible();

  await codingApproval.check();
  await expect(runButton).toBeEnabled();
  await runButton.click();

  await expect.poll(async () => {
    const invocations = await recordedInvocations(page);
    return invocations.findLast((entry) => entry.cmd === "orchestrate_strategy_run")?.args ?? null;
  }).toMatchObject({
    expectedPlanFingerprint: "plan-fixture-refreshed-b5f101cc",
    approvalPlanFingerprint: "plan-fixture-refreshed-b5f101cc",
    approveCodingAgents: true,
  });
});
