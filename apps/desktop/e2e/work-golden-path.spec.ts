import { test, expect } from "@playwright/test";
import { installMockIpc, recordedCommands } from "./mock-ipc";
import { firstRunFixtures, reviewFixtures } from "./fixtures";
import { currentOnboardedFixtures } from "./current-fixtures";

// The golden path through Work: the six-phase rail reflects backend evidence,
// execution previews the exact packet, and review/verification remain owned by
// the canonical Work → Changes → Runs lifecycle rather than a parallel
// orchestration destination. IPC is mocked; phase logic stays covered in Rust.
test.describe("work tab golden path (onboarded)", () => {
  test.beforeEach(async ({ page }) => {
    await installMockIpc(page, currentOnboardedFixtures);
    await page.goto("/");
  });

  test("Work is the default home and renders the six-phase rail", async ({ page }) => {
    await expect(page.getByRole("button", { name: /^Work —/ })).toHaveAttribute("aria-pressed", "true");

    const rail = page.locator(".phase-rail");
    await expect(rail.locator(".phase-chip")).toHaveCount(6);
    for (const title of ["Scope", "Prepare", "Execute", "Review", "Verify", "Finish"]) {
      await expect(rail.getByText(title, { exact: true })).toBeVisible();
    }
    await expect(rail.locator(".phase-current")).toContainText("Execute");
    await expect(rail.locator(".phase-done")).toHaveCount(2);
  });

  test("shows a single primary CTA for the current phase", async ({ page }) => {
    await expect(page.locator(".phase-rail")).toBeVisible();
    const cta = page.locator(".work-cta-row .primary-cta");
    await expect(cta).toHaveCount(1);
    await expect(cta).toHaveText("Run agent");
  });

  test("execution mode toggle switches Agent run ↔ Manual handoff", async ({ page }) => {
    const modeGroup = page.locator(".execution-mode");
    await expect(modeGroup).toBeVisible();
    await expect(modeGroup.getByRole("button", { name: /Agent run/ })).toHaveClass(/selected/);

    await modeGroup.getByRole("button", { name: /Manual handoff/ }).click();
    await expect(page.locator(".work-cta-row .primary-cta")).toHaveText("Generate context pack");

    const commands = await recordedCommands(page);
    expect(commands).toContain("work_phase_state");
    expect(commands).toContain("work_set_execution_mode");
  });

  test("Execute previews the strategy packet and gates launch on required approvals", async ({ page }) => {
    const strategy = page.getByRole("region", { name: "AI execution strategy" });
    const packet = page.getByRole("region", { name: "Execution packet preview" });

    await expect(strategy.getByText("Auto → Lean")).toBeVisible();
    await expect(strategy.getByText("3 → 1")).toBeVisible();
    await expect(packet.locator(".exec-packet-heading strong")).toHaveText("Codex CLI · codex");
    await expect(packet.getByText("Isolated", { exact: true })).toBeVisible();
    await expect(packet.getByText(/4,200 \/ 8,000/)).toBeVisible();

    await expect(page.getByText("Coding agent + isolated writes")).toBeVisible();
    await expect(page.getByText("Paid provider spend")).toBeVisible();

    const cta = page.locator(".work-cta-row .primary-cta");
    await expect(cta).toBeDisabled();
    await page.getByRole("checkbox", { name: /Coding agent \+ isolated writes/ }).check();
    await expect(cta).toBeEnabled();

    await cta.click();
    const commands = await recordedCommands(page);
    expect(commands).toContain("work_strategy_execution_preview");
    expect(commands).toContain("orchestrate_strategy_run");
  });

  test("execution stays inside Work instead of exposing a parallel Orchestrate destination", async ({ page }) => {
    await expect(page.getByRole("button", { name: "Advanced orchestration" })).toHaveCount(0);
    await page.getByRole("button", { name: "Command palette" }).click();
    const input = page.getByRole("textbox", { name: "Search commands" });
    await input.fill("Orchestrate");
    await expect(page.getByText("No matching command")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("button", { name: /^Work —/ })).toHaveAttribute("aria-pressed", "true");
  });

  test("primary activity rail stays focused and utilities are explicit", async ({ page }) => {
    for (const tab of ["Work", "Code", "Changes", "Runs", "Projects"]) {
      await expect(page.getByRole("button", { name: new RegExp(`^${tab} —`) })).toBeVisible();
    }

    await page.getByRole("button", { name: "Command palette" }).click();
    await page.getByRole("textbox", { name: "Search commands" }).fill("Go to Settings");
    await page.keyboard.press("Enter");
    await expect(page.getByRole("heading", { name: "API keys, providers, and preferences." })).toBeVisible();
    await expect(page.getByText("This view crashed")).toHaveCount(0);

    await page.getByRole("button", { name: /^Changes —/ }).click();
    await expect(page.getByText("feat/n2-e2e", { exact: true })).toBeVisible();
    await expect(page.getByRole("region", { name: "Changed files" })).toBeVisible();
    await expect(page.getByText("This view crashed")).toHaveCount(0);

    await page.getByRole("button", { name: /^Runs —/ }).click();
    await expect(page.getByRole("button", { name: /^Runs —/ })).toHaveAttribute("aria-pressed", "true");
    await expect(page.getByText("This view crashed")).toHaveCount(0);
  });
});

test.describe("work tab review (commit visibility + memory)", () => {
  test("Review shows what changed and the proposed memory", async ({ page }) => {
    await installMockIpc(page, reviewFixtures);
    await page.goto("/");
    await expect(page.locator(".phase-rail .phase-current")).toContainText("Review");

    await expect(page.getByText("What changed")).toBeVisible();
    await expect(page.getByText(/src\/app\.ts/).first()).toBeVisible();

    await expect(page.getByText("Add to memory")).toBeVisible();
    await expect(page.getByText(/Remember the auth rate-limit/)).toBeVisible();
    await expect(page.getByRole("button", { name: "Accept" }).first()).toBeVisible();

    const commands = await recordedCommands(page);
    expect(commands).toContain("orchestrate_run_diffs");
    expect(commands).toContain("memory_proposals_list");
  });

  test("Review has evidence-bound Accept/Reject and no manual bypass", async ({ page }) => {
    await installMockIpc(page, reviewFixtures);
    await page.goto("/");
    await expect(page.locator(".phase-rail .phase-current")).toContainText("Review");

    await expect(page.getByRole("button", { name: "Mark reviewed" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Mark committed" })).toHaveCount(0);

    await page.getByRole("button", { name: /Accept .* Verify/ }).click();
    await expect.poll(async () => await recordedCommands(page)).toContain("work_review");
  });
});

test.describe("work tab scope onboarding (first run)", () => {
  test("Scope phase onboards from the Work tab itself", async ({ page }) => {
    await installMockIpc(page, firstRunFixtures);
    await page.goto("/");
    const rail = page.locator(".phase-rail");
    await expect(rail.locator(".phase-current")).toContainText("Scope");
    await expect(page.getByRole("button", { name: "Connect a project" })).toBeVisible();
  });
});
