import { test, expect, type Page } from "@playwright/test";
import { installMockIpc, recordedCommands } from "./mock-ipc";
import { onboardedFixtures } from "./fixtures";

// The classic Workflow surface moved under "Advanced" in the redesigned nav
// (the Work tab is the new default home). Open it explicitly where a test
// asserts Workflow internals.
async function openWorkflow(page: Page) {
  const nav = page.locator(".nav-list");
  await nav.getByRole("button", { name: /^Advanced/ }).click();
  await nav.getByRole("button", { name: /^Workflow/ }).click();
}

// Drives the daily loop on a fully onboarded, commit-ready workspace:
// shell boots → workflow home → dashboard → git → code, asserting the key UI
// state at each surface. IPC is mocked, so this asserts the frontend wiring
// (hooks → queries → render), not the Rust backend.
test.describe("daily loop (onboarded)", () => {
  test.beforeEach(async ({ page }) => {
    await installMockIpc(page, onboardedFixtures);
    await page.goto("/");
  });

  test("shell boots with active workspace + status strip", async ({ page }) => {
    await expect(page.getByText("AI control cockpit")).toBeVisible();
    // Header shows the active project name from get_active_project_config.
    await expect(page.getByRole("heading", { level: 2, name: "RepoDesk" })).toBeVisible();
    // Status strip reflects the mocked snapshot. Scope to the header: the task
    // title also appears in the workflow TaskSwitcher panel.
    await expect(page.locator("header").getByText("Wire N2 E2E smoke")).toBeVisible();
    await expect(page.getByText("3 changes")).toBeVisible();
    await expect(page.getByText("1/4 working")).toBeVisible();
  });

  test("workflow home shows a commit-ready next step and route", async ({ page }) => {
    await openWorkflow(page);
    await expect(page.getByRole("heading", { level: 1, name: "Build smart context" })).toBeVisible();
    // Commit readiness panel reflects the "ready" status.
    await expect(page.getByText("All checks passed — safe to commit")).toBeVisible();
    await expect(page.getByText("Ready to commit")).toBeVisible();
    // Best route panel rendered from routing_snapshot.
    await expect(page.getByRole("heading", { name: "ollama / llama3" })).toBeVisible();
    // The "do next safe step" CTA is enabled (not blocked by onboarding).
    await expect(page.getByRole("button", { name: "Build bounded context" })).toBeEnabled();
  });

  test("journey stepper makes the 8-step path legible", async ({ page }) => {
    await openWorkflow(page);
    // The stepper is the spine of the classic Workflow surface.
    await expect(page.getByRole("heading", { name: "One task, eight steps" })).toBeVisible();
    // All 8 steps render, including blocked ones downstream.
    const track = page.locator(".journey-track");
    await expect(track.locator(".journey-node")).toHaveCount(8);
    // The current step (smart_context) is highlighted.
    await expect(track.locator(".journey-node.current")).toContainText("Smart Context");
    // Auto-vs-manual is explicit on every step via compact tags.
    await expect(track.locator(".journey-tag")).toHaveCount(8);
    await expect(track.getByText("Auto", { exact: true }).first()).toBeVisible();
    await expect(track.getByText("You", { exact: true }).first()).toBeVisible();
    // The focused step spells out who performs it in full.
    await expect(page.locator(".journey-detail").getByText("RepoDesk does this")).toBeVisible();
    // The primary CTA previews what it will run, before clicking.
    await expect(page.locator(".cta-preview")).toContainText("This runs");
    await expect(page.locator(".cta-preview")).toContainText("Smart Context");
  });

  test("navigates every tab without crashing", async ({ page }) => {
    // Visit all surfaces — partial mock data must render an empty state, never
    // the error boundary ("This view crashed" / "Something went wrong").
    // Primary spine is collapsed to four surfaces; depth lives under "Advanced".
    const nav = page.locator(".nav-list");
    const primaryTabs = ["Work", "Changes", "History", "Settings"];
    const moreTabs = ["Workflow", "Dashboard", "Git", "Code", "Orchestrate", "Memory", "Models", "Tokens", "System Registry", "Debug"];
    for (const tab of primaryTabs) {
      await nav.getByRole("button", { name: new RegExp(`^${tab}`) }).click();
      await expect(page.locator(".app-shell")).toBeVisible();
      await expect(page.getByText("This view crashed")).toHaveCount(0);
      await expect(page.getByText("Something went wrong")).toHaveCount(0);
    }
    // Expand the "Advanced" section to reach depth & diagnostics surfaces.
    await nav.getByRole("button", { name: /^Advanced/ }).click();
    for (const tab of moreTabs) {
      await nav.getByRole("button", { name: new RegExp(`^${tab}`) }).click();
      await expect(page.locator(".app-shell")).toBeVisible();
      await expect(page.getByText("This view crashed")).toHaveCount(0);
      await expect(page.getByText("Something went wrong")).toHaveCount(0);
    }
  });

  test("the hero next-step CTA reports its result too", async ({ page }) => {
    await openWorkflow(page);
    // Clicking the big "do next safe step" CTA surfaces the same human result
    // on the matching step card as running it from the card would.
    await page.getByRole("button", { name: "Build bounded context" }).click();
    await expect(page.locator(".journey-detail").getByText(/Smart Context done\./)).toBeVisible();
  });

  test("a journey step can be run from its card with a result", async ({ page }) => {
    await openWorkflow(page);
    // The focused (current) step card offers a run action…
    const detail = page.locator(".journey-detail");
    const runBtn = detail.getByRole("button", { name: /^Run Smart Context/ });
    await expect(runBtn).toBeVisible();
    await runBtn.click();
    // …and reports a human-readable result inline.
    await expect(detail.getByText(/Smart Context done\./)).toBeVisible();
  });

  test("Models tab guides setup with human status and fixes", async ({ page }) => {
    const nav = page.locator(".nav-list");
    await nav.getByRole("button", { name: /^Advanced/ }).click();
    await nav.getByRole("button", { name: /^Models/ }).click();
    // Readiness-focused headline + attention banner (1 working, 2 need attention).
    await expect(page.getByRole("heading", { name: /Ready for AI/ })).toBeVisible();
    await expect(page.getByText(/need.* attention/i)).toBeVisible();
    // Human statuses replace raw reachability strings.
    await expect(page.getByText("Ready", { exact: true }).first()).toBeVisible();
    await expect(page.getByText("Needs API key")).toBeVisible();
    // Concrete one-click fixes are offered.
    await expect(page.getByRole("button", { name: "Add key" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Launch app" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Turn on" })).toBeVisible();
  });

  test("command palette opens with Ctrl-K and navigates", async ({ page }) => {
    await openWorkflow(page);
    // Wait for the app to mount (so the global keydown listener is attached).
    await expect(page.getByRole("heading", { level: 1, name: "Build smart context" })).toBeVisible();
    await page.locator("body").click();
    await page.keyboard.press("ControlOrMeta+k");
    const input = page.getByPlaceholder("Search tabs and actions…");
    await expect(input).toBeVisible();
    await input.fill("Git");
    await page.keyboard.press("Enter");
    await expect(page.getByRole("heading", { level: 1, name: "feat/n2-e2e" })).toBeVisible();
  });

  test("project switcher lists connected projects", async ({ page }) => {
    await page.getByRole("button", { name: /RepoDesk/ }).first().click();
    await expect(page.getByRole("button", { name: /my-api/ })).toBeVisible();
    await expect(page.getByRole("button", { name: /Open from folder/ })).toBeVisible();
    await expect(page.getByRole("button", { name: /Connect with details/ })).toBeVisible();
  });

  test("frontend actually issued the daily-loop commands through IPC", async ({ page }) => {
    await openWorkflow(page);
    // Give the React Query hooks a beat to fire.
    await expect(page.getByText("All checks passed — safe to commit")).toBeVisible();
    const commands = await recordedCommands(page);
    expect(commands).toContain("desktop_snapshot");
    expect(commands).toContain("product_workflow_state");
    expect(commands).toContain("git_workspace_snapshot");
    expect(commands).toContain("model_health_snapshot");
  });

  test("orchestrate shows reviewable agent changes with proof", async ({ page }) => {
    const nav = page.locator(".nav-list");
    await nav.getByRole("button", { name: /^Advanced/ }).click();
    await nav.getByRole("button", { name: /^Orchestrate/ }).click();
    await expect(page.getByRole("heading", { name: /Conduct sub-agents/ })).toBeVisible();
    await expect(page.getByRole("heading", { name: /agent-changed file/ })).toBeVisible();
    await expect(page.getByText("Checks passed")).toBeVisible();
    await expect(page.getByText("verify passed").first()).toBeVisible();
    await expect(page.getByText("src/app.ts").first()).toBeVisible();
    await expect(page.locator(".diff-add").getByText("+new line")).toBeVisible();
    const commandsBeforeAccept = await recordedCommands(page);
    const checksBeforeAccept = commandsBeforeAccept.filter((command) => command === "orchestrate_check_proof").length;
    await page.getByRole("button", { name: /Accept & run checks \(1\)/ }).click();
    await expect(page.getByText("Accepted src/app.ts (applied and staged)").first()).toBeVisible();
    await expect(page.getByRole("button", { name: "Reject changes" })).toBeVisible();
    const commandsAfterAccept = await recordedCommands(page);
    expect(commandsAfterAccept).toContain("orchestrate_review");
    expect(commandsAfterAccept.filter((command) => command === "orchestrate_check_proof").length).toBeGreaterThan(checksBeforeAccept);
  });
});

// Once prompts are generated, the home must spell out the manual finish loop:
// copy the prompt → hand to your agent → capture into Memory → commit.
test.describe("finish line (prompts ready)", () => {
  test("guides the hand-off and commit", async ({ page }) => {
    await installMockIpc(page, {
      ...onboardedFixtures,
      product_workflow_state: { ...(onboardedFixtures.product_workflow_state as object), prompts_ok: true },
    });
    await page.goto("/");
    await openWorkflow(page);
    await expect(page.getByRole("heading", { name: "Hand off & complete the task" })).toBeVisible();
    // Explains the prompt is built locally (trust), and points concretely at the panel.
    await expect(page.getByText(/assembled your prompt/)).toBeVisible();
    await expect(page.getByText(/Copy the prompt/)).toBeVisible();
    await expect(page.getByRole("button", { name: /Show the prompt/ })).toBeVisible();
    await expect(page.getByRole("button", { name: /Open Memory/ })).toBeVisible();
    // The Generated Prompts panel is right below, with the local-build note.
    await expect(page.locator("#generated-prompts")).toBeVisible();
    await expect(page.getByText(/Built locally from your bounded context/)).toBeVisible();
    await page.getByRole("button", { name: "Context Pack" }).click();
    await expect(page.getByText("RepoDesk Agent Context Pack")).toBeVisible();
    await expect(page.getByText("Operating Rules For The Agent")).toBeVisible();
  });
});
