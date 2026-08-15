import { expect, test } from "@playwright/test";
import { currentOnboardedFixtures } from "./current-fixtures";
import { installMockIpc, recordedInvocations } from "./mock-ipc";

const successResult = {
  ok: true,
  command: "",
  stdout: "",
  stderr: "",
  exit_code: 0,
};

test("Projects Registry owns repository registration and activation", async ({ page }) => {
  await installMockIpc(page, {
    ...currentOnboardedFixtures,
    project_list_configs: [],
    project_add: { ...successResult, command: "project add" },
    project_use: { ...successResult, command: "project use" },
  });
  await page.goto("/");

  await page.getByRole("button", { name: /^Projects —/ }).click();
  await expect(page.getByRole("heading", { name: "Repository workspaces" })).toBeVisible();
  await expect(page.getByText("No projects registered.")).toBeVisible();

  await page.getByRole("button", { name: "Add project" }).click();
  await expect(page.getByRole("heading", { name: "Add and activate a project" })).toBeVisible();

  await page.getByLabel("Project name").fill("new-repo");
  await page.getByLabel("Project path").fill("/Users/you/code/new-repo");
  await page.getByLabel("Project type").fill("repository");
  await page.getByLabel("Main language").fill("typescript");
  await page.getByRole("button", { name: "Add and activate project" }).click();

  await expect(page.getByText('Project "new-repo" was added and activated.')).toBeVisible();

  const projectInvocations = (await recordedInvocations(page)).filter(({ cmd }) =>
    cmd === "project_add" || cmd === "project_use",
  );
  expect(projectInvocations).toEqual([
    {
      cmd: "project_add",
      args: {
        input: {
          name: "new-repo",
          path: "/Users/you/code/new-repo",
          project_type: "repository",
          main_language: "typescript",
        },
      },
    },
    { cmd: "project_use", args: { name: "new-repo" } },
  ]);
});
