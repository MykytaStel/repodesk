import { expect, test, type Page } from "@playwright/test";
import { currentOnboardedFixtures } from "./current-fixtures";
import { installMockIpc } from "./mock-ipc";

async function openDirectly(page: Page, route: string, viewport = { width: 1280, height: 800 }) {
  await page.setViewportSize(viewport);
  await page.addInitScript((activeRoute) => {
    window.localStorage.setItem("repodesk.activeTab", activeRoute);
  }, route);
  await installMockIpc(page, currentOnboardedFixtures);
  await page.goto("/");
}

async function gridColumnCount(page: Page, selector: string) {
  return page.locator(selector).evaluate((element) =>
    getComputedStyle(element).gridTemplateColumns.split(" ").filter(Boolean).length,
  );
}

test("secondary route CSS is ready on direct entry", async ({ page }) => {
  await openDirectly(page, "orchestrate");
  await expect(page.getByRole("heading", { name: /Run sub-agents/ })).toBeVisible();
  expect(await gridColumnCount(page, ".orchestrate-control-panel")).toBe(3);

  await openDirectly(page, "dashboard", { width: 760, height: 720 });
  await expect(page.getByRole("heading", { name: "Project state, context, and verification evidence." })).toBeVisible();
  expect(await page.locator(".route-panel").evaluate((element) => getComputedStyle(element).display)).toBe("grid");
  expect(await gridColumnCount(page, ".route-summary-grid")).toBe(1);
});

test("moved product styles still yield to the Work workbench layer", async ({ page }) => {
  await openDirectly(page, "work");
  await expect(page.getByRole("group", { name: "Execution mode" })).toBeVisible();

  expect(await page.locator(".work-focus-card").evaluate((element) => getComputedStyle(element).borderRadius)).toBe("9px");
  expect(await gridColumnCount(page, ".work-current-step-copy")).toBe(2);

  await page.setViewportSize({ width: 680, height: 720 });
  expect(await gridColumnCount(page, ".work-current-step-copy")).toBe(1);
});

test("Runs shared subnavigation is styled on direct entry", async ({ page }) => {
  await openDirectly(page, "history");
  const subnav = page.getByRole("tablist", { name: "Runs views" });
  const selected = subnav.getByRole("tab", { selected: true });

  await expect(subnav).toBeVisible();
  expect(await subnav.evaluate((element) => getComputedStyle(element).display)).toBe("flex");
  expect(await selected.evaluate((element) => getComputedStyle(element).backgroundColor)).not.toBe(
    "rgba(0, 0, 0, 0)",
  );
});

test("Models & Cost shared subnavigation is styled on direct entry", async ({ page }) => {
  await openDirectly(page, "models-cost");
  const subnav = page.getByRole("tablist", { name: "Models and cost views" });
  const selected = subnav.getByRole("tab", { selected: true });

  await expect(subnav).toBeVisible();
  expect(await subnav.evaluate((element) => getComputedStyle(element).display)).toBe("flex");
  expect(await selected.evaluate((element) => getComputedStyle(element).backgroundColor)).not.toBe(
    "rgba(0, 0, 0, 0)",
  );
});

test("Playbooks shared manual import control is styled on direct entry", async ({ page }) => {
  await openDirectly(page, "playbooks");
  const input = page.locator(".manual-import-input");

  await expect(input).toBeVisible();
  expect(await input.evaluate((element) => getComputedStyle(element).fontFamily)).toContain("SFMono-Regular");
});
