import { test, expect } from "@playwright/test";
import { installMockIpc, recordedCommands } from "./mock-ipc";
import { onboardedFixtures, firstRunFixtures, reviewFixtures } from "./fixtures";

// The golden path through the redesigned Work tab: the app lands on Work, the
// six-phase rail reflects the backend progression, the single primary CTA and
// the execution-mode toggle are present, switching mode drives the backend, and
// the full orchestrator controls live one disclosure away. IPC is mocked, so
// this asserts the Work surface wiring (the phase *logic* is covered by the Rust
// unit/integration tests), not the backend.
test.describe("work tab golden path (onboarded)", () => {
  test.beforeEach(async ({ page }) => {
    await installMockIpc(page, onboardedFixtures);
    await page.goto("/");
  });

  test("Work is the default home and renders the six-phase rail", async ({ page }) => {
    // Work is the primary spine's first tab and the default landing. Scope to
    // `.nav-item` so the "Work" tab isn't confused with the "Work" group toggle.
    await expect(
      page.locator(".nav-item").filter({ has: page.getByText("Work", { exact: true }) }),
    ).toHaveClass(/active/);

    // The phase rail shows all six phases in order.
    const rail = page.locator(".phase-rail");
    await expect(rail.locator(".phase-chip")).toHaveCount(6);
    for (const title of ["Scope", "Prepare", "Execute", "Review", "Verify", "Finish"]) {
      await expect(rail.getByText(title, { exact: true })).toBeVisible();
    }
    // The current phase (Execute) is marked, and completed phases read done.
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
    // Agent run is selected initially (from work_phase_state).
    await expect(modeGroup.getByRole("button", { name: /Agent run/ })).toHaveClass(/selected/);

    // Switching to Manual handoff drives the backend and updates the CTA.
    await modeGroup.getByRole("button", { name: /Manual handoff/ }).click();
    await expect(page.locator(".work-cta-row .primary-cta")).toHaveText("Generate context pack");

    const commands = await recordedCommands(page);
    expect(commands).toContain("work_phase_state");
    expect(commands).toContain("work_set_execution_mode");
  });

  test("Execute previews the run and gates launch on required approvals", async ({ page }) => {
    // The pre-launch preview spells out executor/model/workspace/writes/cost.
    await expect(page.getByText("Before you launch")).toBeVisible();
    await expect(page.locator(".exec-preview-grid").getByText("Codex CLI")).toBeVisible();
    await expect(page.locator(".exec-preview-grid").getByText("Isolated worktree")).toBeVisible();

    // Agent-run mode surfaces the ExecutionAuthorization gates on the card.
    await expect(page.getByText(/Approve coding-agent CLIs/)).toBeVisible();
    await expect(page.getByText(/Approve paid providers/)).toBeVisible();

    // The run needs the coding-agent approval, so the CTA is blocked until granted.
    const cta = page.locator(".work-cta-row .primary-cta");
    await expect(cta).toBeDisabled();
    await page.getByRole("checkbox", { name: /Approve coding-agent CLIs/ }).check();
    await expect(cta).toBeEnabled();

    // Now the primary CTA launches the orchestrator run inline.
    await cta.click();
    const commands = await recordedCommands(page);
    expect(commands).toContain("work_execution_preview");
    expect(commands).toContain("orchestrate_run");
  });

  test("advanced orchestrator details stay collapsed until disclosed", async ({ page }) => {
    const disclosure = page.getByRole("button", { name: /Advanced orchestrator details/ });
    await expect(disclosure).toHaveAttribute("aria-expanded", "false");
    await disclosure.click();
    await expect(disclosure).toHaveAttribute("aria-expanded", "true");
  });

  test("primary nav is collapsed to Work / Changes / History / Settings", async ({ page }) => {
    const nav = page.locator(".nav-list .nav-group").first();
    for (const tab of ["Work", "Changes", "History", "Settings"]) {
      await expect(nav.getByRole("button", { name: new RegExp(`^${tab}`) })).toBeVisible();
    }
    // Changes is one unified surface: a workspace summary header above a single
    // changed-files list + preview pane (no segmented Git/Code subnav).
    await nav.getByRole("button", { name: /^Changes/ }).click();
    await expect(page.locator(".changes-summary")).toBeVisible();
    await expect(page.locator(".changes-summary").getByText("feat/n2-e2e")).toBeVisible();
    await expect(page.getByRole("heading", { name: "Changed files" })).toBeVisible();
    await expect(page.getByText("This view crashed")).toHaveCount(0);

    await nav.getByRole("button", { name: /^History/ }).click();
    await expect(page.locator(".changes-summary")).toBeVisible();
    await expect(page.locator(".subnav")).toBeVisible();
    await expect(page.getByText("This view crashed")).toHaveCount(0);
  });
});

test.describe("work tab review (commit visibility + memory)", () => {
  test("Review shows what changed and the proposed memory", async ({ page }) => {
    await installMockIpc(page, reviewFixtures);
    await page.goto("/");
    await expect(page.locator(".phase-rail .phase-current")).toContainText("Review");

    // Commit visibility: the changed file and its diff are right here.
    await expect(page.getByText("What changed")).toBeVisible();
    await expect(page.getByText(/src\/app\.ts/).first()).toBeVisible();

    // Memory: the run's proposed capture, acceptable inline (= add to memory).
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

    // The phase advances only through accept/reject — the old "Mark reviewed"
    // and "Mark committed" bypass buttons are gone.
    await expect(page.getByRole("button", { name: "Mark reviewed" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Mark committed" })).toHaveCount(0);

    // Accept drives the atomic, evidence-bound backend review (which records the
    // Accepted receipt and advances the phase server-side).
    await page.getByRole("button", { name: /Accept .* Verify/ }).click();
    await expect.poll(async () => await recordedCommands(page)).toContain("work_review");
  });
});

test.describe("work tab scope onboarding (first run)", () => {
  test("Scope phase onboards from the Work tab itself", async ({ page }) => {
    await installMockIpc(page, firstRunFixtures);
    await page.goto("/");
    // Work is the default home even with no project; Scope is the live phase.
    const rail = page.locator(".phase-rail");
    await expect(rail.locator(".phase-current")).toContainText("Scope");
    // Onboarding starts right here — no detour to the legacy Workflow surface.
    await expect(page.getByRole("button", { name: "Connect a project" })).toBeVisible();
  });
});
