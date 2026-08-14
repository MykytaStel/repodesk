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
  return page.locator(selector).first().evaluate((element) =>
    getComputedStyle(element).gridTemplateColumns.split(" ").filter(Boolean).length,
  );
}

test("historical product routes migrate to canonical owners on direct entry", async ({ page }) => {
  await openDirectly(page, "orchestrate");
  await expect(page.getByRole("button", { name: /^Work —/ })).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByRole("group", { name: "Execution mode" })).toBeVisible();
  await expect.poll(() => page.evaluate(() => window.localStorage.getItem("repodesk.activeTab"))).toBe("work");

  await openDirectly(page, "dashboard", { width: 760, height: 720 });
  await expect(page.getByRole("button", { name: /^Work —/ })).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator(".phase-rail")).toBeVisible();

  await openDirectly(page, "models-cost");
  await expect(page.getByRole("heading", { name: "API keys, providers, and workspace." })).toBeVisible();
  await expect.poll(() => page.evaluate(() => window.localStorage.getItem("repodesk.activeTab"))).toBe("settings");
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
  expect(await selected.evaluate((element) => getComputedStyle(element).backgroundColor)).not.toBe("rgba(0, 0, 0, 0)");
});

test("Projects owns lazy Knowledge and Work Template styling", async ({ page }) => {
  await openDirectly(page, "projects");
  const subnav = page.getByRole("tablist", { name: "Project views" });
  await expect(subnav).toBeVisible();

  await subnav.getByRole("tab", { name: "Work templates" }).click();
  const input = page.locator(".manual-import-input");
  await expect(input).toBeVisible();
  expect(await input.evaluate((element) => getComputedStyle(element).fontFamily)).toContain("SFMono-Regular");
  expect(await gridColumnCount(page, ".playbook-route")).toBe(3);

  await subnav.getByRole("tab", { name: "Knowledge" }).click();
  await expect(page.getByRole("heading", { name: "Engineering knowledge" })).toBeVisible();
});
