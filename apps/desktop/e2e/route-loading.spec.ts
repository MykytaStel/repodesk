import { expect, test, type Locator, type Page } from "@playwright/test";
import { TAB_IDS } from "../src/app/constants";
import { currentOnboardedFixtures } from "./current-fixtures";
import { installMockIpc } from "./mock-ipc";

const routes = [
  ["work", "Work"],
  ["code", "Code"],
  ["changes", "Changes"],
  ["history", "Runs"],
  ["projects", "Projects"],
  ["dashboard", "Dashboard"],
  ["tokens", "Tokens"],
  ["models", "Models"],
  ["git", "Git"],
  ["memory", "Knowledge"],
  ["orchestrate", "Orchestrate"],
  ["outcomes", "Outcomes"],
  ["playbooks", "Playbooks"],
  ["models-cost", "Models & Cost"],
  ["audit", "Audit"],
  ["settings", "Settings"],
  ["system", "System Registry"],
  ["debug", "Debug"],
] as const;

type RouteId = (typeof routes)[number][0];

const readySurface: Record<RouteId, (page: Page) => Locator> = {
  work: (page) => page.getByRole("group", { name: "Execution mode" }),
  code: (page) => page.getByRole("toolbar", { name: "Code workspace actions" }),
  changes: (page) => page.getByRole("region", { name: "Changed files" }),
  history: (page) => page.getByRole("tablist", { name: "Runs views" }),
  projects: (page) => page.getByRole("heading", { name: "Repository workspaces" }),
  dashboard: (page) => page.getByRole("heading", { name: "Project state, context, and verification evidence." }),
  tokens: (page) => page.getByRole("heading", { name: /total tokens logged\.$/ }),
  models: (page) => page.getByRole("heading", { name: /(?:Ready for AI|No models ready yet)/ }),
  git: (page) => page.getByRole("heading", { name: "feat/n2-e2e" }),
  memory: (page) => page.getByRole("heading", { name: "Engineering knowledge" }),
  orchestrate: (page) => page.getByRole("heading", { name: /Run sub-agents for/ }),
  outcomes: (page) => page.getByRole("heading", { name: "Outcome ledger" }),
  playbooks: (page) => page.getByRole("heading", { name: "Workflow shortcuts" }),
  "models-cost": (page) => page.getByRole("tablist", { name: "Models and cost views" }),
  audit: (page) => page.getByRole("heading", { name: /hash chain\.$/ }),
  settings: (page) => page.getByRole("heading", { name: "API keys, providers, and preferences." }),
  system: (page) => page.getByRole("heading", { name: "Agent skills & context boundaries" }),
  debug: (page) => page.getByRole("table", { name: "Instrumented IPC runtime metrics" }),
};

const primaryLandmark: Partial<Record<RouteId, (page: Page) => Locator>> = {
  work: (page) => page.getByRole("region", { name: "Current Work Item" }),
  code: (page) => page.getByRole("tree", { name: "Repository files" }),
  changes: (page) => page.getByRole("region", { name: "Changed files" }),
  history: (page) => page.getByRole("tablist", { name: "Runs views" }),
  projects: (page) => page.getByRole("heading", { name: "Repository workspaces" }),
};

test("startup matrix covers every registered persisted route", () => {
  expect(routes.map(([route]) => route)).toEqual([...TAB_IDS]);
});

for (const [route, title] of routes) {
  test(`cold startup restores ${route}`, async ({ page }) => {
    const pageErrors: string[] = [];
    const nativeDialogs: string[] = [];
    page.on("pageerror", (error) => pageErrors.push(error.message));
    page.on("dialog", (dialog) => {
      nativeDialogs.push(`${dialog.type()}: ${dialog.message()}`);
      void dialog.dismiss();
    });

    await page.addInitScript((activeRoute) => {
      window.localStorage.setItem("repodesk.activeTab", activeRoute);
    }, route);
    await installMockIpc(page, currentOnboardedFixtures);
    await page.goto("/");

    const breadcrumb = page.getByLabel("Current workspace location");
    await expect(breadcrumb.getByText(title, { exact: true })).toBeVisible();
    await expect(readySurface[route](page)).toBeVisible();
    await expect(page.locator("main.ide-surface-scroll .skeleton-panel")).toHaveCount(0);
    await expect.poll(async () => (await page.locator("main.ide-surface-scroll").innerText()).trim().length).toBeGreaterThan(0);
    await expect(page.getByRole("heading", { name: /(?:This view crashed|RepoDesk hit an unexpected error)/ })).toHaveCount(0);

    const landmark = primaryLandmark[route];
    if (landmark) await expect(landmark(page)).toBeVisible();

    expect(pageErrors).toEqual([]);
    expect(nativeDialogs).toEqual([]);
  });
}
