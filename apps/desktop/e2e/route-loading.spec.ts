import { expect, test, type Locator, type Page } from "@playwright/test";
import { LEGACY_TAB_ALIASES, TAB_IDS } from "../src/app/constants";
import { currentOnboardedFixtures } from "./current-fixtures";
import { installMockIpc } from "./mock-ipc";

const routes = [
  ["work", "Work"],
  ["code", "Code"],
  ["changes", "Changes"],
  ["history", "Runs"],
  ["projects", "Projects"],
  ["settings", "Settings"],
  ["debug", "Debug"],
  ["dashboard", "Work"],
  ["tokens", "Settings"],
  ["models", "Settings"],
  ["git", "Changes"],
  ["memory", "Projects"],
  ["orchestrate", "Work"],
  ["outcomes", "Runs"],
  ["playbooks", "Projects"],
  ["models-cost", "Settings"],
  ["audit", "Runs"],
  ["system", "Settings"],
] as const;

type RouteId = (typeof routes)[number][0];
type CanonicalRouteId = "work" | "code" | "changes" | "history" | "projects" | "settings" | "debug";

function canonicalRoute(route: RouteId): CanonicalRouteId {
  return (LEGACY_TAB_ALIASES[route] ?? route) as CanonicalRouteId;
}

const readySurface: Record<CanonicalRouteId, (page: Page) => Locator> = {
  work: (page) => page.getByRole("group", { name: "Execution mode" }),
  code: (page) => page.getByRole("toolbar", { name: "Code workspace actions" }),
  changes: (page) => page.getByRole("region", { name: "Changed files" }),
  history: (page) => page.getByRole("tablist", { name: "Runs views" }),
  projects: (page) => page.getByRole("heading", { name: "Repository workspaces" }),
  settings: (page) => page.getByRole("heading", { name: "API keys, providers, and preferences." }),
  debug: (page) => page.getByRole("table", { name: "Instrumented IPC runtime metrics" }),
};

const primaryLandmark: Partial<Record<CanonicalRouteId, (page: Page) => Locator>> = {
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
    const canonical = canonicalRoute(route);
    await expect(readySurface[canonical](page)).toBeVisible();
    await expect(page.locator("main.ide-surface-scroll .skeleton-panel")).toHaveCount(0);
    await expect.poll(async () => (await page.locator("main.ide-surface-scroll").innerText()).trim().length).toBeGreaterThan(0);
    await expect(page.getByRole("heading", { name: /(?:This view crashed|RepoDesk hit an unexpected error)/ })).toHaveCount(0);

    const landmark = primaryLandmark[canonical];
    if (landmark) await expect(landmark(page)).toBeVisible();

    expect(pageErrors).toEqual([]);
    expect(nativeDialogs).toEqual([]);
  });
}
