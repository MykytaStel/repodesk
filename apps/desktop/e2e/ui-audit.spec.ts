import { expect, test, type Page } from "@playwright/test";
import { currentOnboardedFixtures } from "./current-fixtures";
import { installMockIpc } from "./mock-ipc";

const primaryTabs = ["Work", "Code", "Changes", "Runs", "Projects"];

async function waitForPrimaryRoute(page: Page, tab: string) {
  switch (tab) {
    case "Work":
      await expect(page.getByRole("group", { name: "Execution mode" })).toBeVisible();
      break;
    case "Code":
      await expect(page.getByRole("toolbar", { name: "Code workspace actions" })).toBeVisible();
      break;
    case "Changes":
      await expect(page.getByRole("region", { name: "Changed files" })).toBeVisible();
      break;
    case "Runs":
      await expect(page.getByRole("tablist", { name: "Runs views" })).toBeVisible();
      await expect(page.locator(".runs-shell")).toBeVisible();
      break;
    case "Projects":
      await expect(page.getByRole("heading", { name: "Repository workspaces" })).toBeVisible();
      await expect(page.locator(".project-registry-grid")).toBeVisible();
      break;
  }
}

async function openFromPalette(page: Page, title: string) {
  await page.getByRole("button", { name: "Command palette" }).click();
  await page.getByRole("textbox", { name: "Search commands" }).fill(title);
  await page.keyboard.press("Enter");
}

async function gridColumnCount(page: Page, selector: string) {
  return page.locator(selector).evaluate((element) =>
    getComputedStyle(element).gridTemplateColumns.split(" ").filter(Boolean).length,
  );
}

for (const viewport of [
  { name: "desktop", width: 1280, height: 800 },
  { name: "narrow", width: 760, height: 720 },
]) {
  test(`primary surfaces fit the ${viewport.name} workspace`, async ({ page }) => {
    const errors: string[] = [];
    page.on("console", (message) => {
      if (message.type() === "error") errors.push(message.text());
    });
    await page.setViewportSize(viewport);
    await installMockIpc(page, currentOnboardedFixtures);
    await page.goto("/");

    for (const tab of primaryTabs) {
      await page.getByRole("button", { name: new RegExp(`^${tab} —`) }).click();
      await waitForPrimaryRoute(page, tab);
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth)).toBe(true);
      expect(await page.locator("main").evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
    }

    expect(errors).toEqual([]);
  });
}

test("shared table primitives are ready before Runs is activated", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await installMockIpc(page, currentOnboardedFixtures);
  await page.goto("/");

  await openFromPalette(page, "System Registry");
  await expect(page.getByRole("heading", { name: "Agent skills & context boundaries" })).toBeVisible();
  await expect(page.getByRole("button", { name: /^Runs —/ })).toHaveAttribute("aria-pressed", "false");

  expect(await page.locator(".table-list").first().evaluate((element) => getComputedStyle(element).display)).toBe("grid");
});

test("Work keeps two columns on desktop and one column at narrow width", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await installMockIpc(page, currentOnboardedFixtures);
  await page.goto("/");
  await waitForPrimaryRoute(page, "Work");

  expect(await gridColumnCount(page, ".work-current-step-copy")).toBe(2);

  await page.setViewportSize({ width: 680, height: 720 });
  expect(await gridColumnCount(page, ".work-current-step-copy")).toBe(1);
  expect(await page.locator(".work-current-step-copy > small").evaluate((element) => getComputedStyle(element).gridColumnStart)).toBe("1");
});

test("Runs workspace adapts from desktop to narrow columns", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await installMockIpc(page, currentOnboardedFixtures);
  await page.goto("/");

  await page.getByRole("button", { name: /^Runs —/ }).click();
  await waitForPrimaryRoute(page, "Runs");
  expect(await gridColumnCount(page, ".runs-shell")).toBe(2);

  await page.setViewportSize({ width: 680, height: 720 });
  expect(await gridColumnCount(page, ".runs-shell")).toBe(1);
});

test("History rows preserve utility layout and narrow metadata alignment", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await installMockIpc(page, {
    ...currentOnboardedFixtures,
    outcomes_stats: [{
      task_kind: "implementation",
      provider: "codex_cli",
      scored_runs: 1,
      good: 1,
      bad: 0,
      neutral: 0,
      success_rate: 1,
      avg_cost_units: 0,
    }],
  });
  await page.goto("/");

  await page.getByRole("button", { name: /^Runs —/ }).click();
  await waitForPrimaryRoute(page, "Runs");

  await page.getByRole("tab", { name: "Provider outcomes" }).click();
  await expect(page.getByRole("heading", { name: "Outcome ledger" })).toBeVisible();
  const outcomeRow = page.locator(".table-row").first();
  expect(await outcomeRow.evaluate((element) => getComputedStyle(element).display)).toBe("flex");

  await page.setViewportSize({ width: 680, height: 720 });
  expect(await outcomeRow.locator(".row-meta").evaluate((element) => getComputedStyle(element).justifyContent)).toBe("flex-start");
});
