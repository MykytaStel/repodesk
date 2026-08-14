import { expect, test } from "@playwright/test";
import { currentOnboardedFixtures } from "./current-fixtures";
import { emitMockTauriEvent, installMockIpc, recordedCommands } from "./mock-ipc";

const optionalResourceFragments = [
  "CommandPalette",
  "command-palette-v2.css",
  "WorkbenchBottomPanel",
  "InteractiveTerminal",
  "vendor-terminal",
  "@xterm",
  "xterm.css",
  "terminal.css",
  "task-runner.css",
  "IDEHealthPanel.tsx",
  "health-panel.css",
];

function matchingResources(resources: string[], fragments: string[]) {
  return resources.filter((url) => fragments.some((fragment) => url.includes(fragment)));
}

async function closePalette(page: import("@playwright/test").Page) {
  await page.getByRole("textbox", { name: "Search commands" }).focus();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("textbox", { name: "Search commands" })).toHaveCount(0);
}

async function openFromPalette(page: import("@playwright/test").Page, title: string) {
  await page.getByRole("button", { name: "Command palette" }).click();
  const input = page.getByRole("textbox", { name: "Search commands" });
  await input.fill(title);
  await page.locator(".cmdk-item").filter({ has: page.getByText(title, { exact: true }) }).click();
}

test("About presents the engineering-workspace identity and restores focus", async ({ page }) => {
  await installMockIpc(page, currentOnboardedFixtures);
  await page.goto("/");

  const opener = page.getByRole("button", { name: "About RepoDesk" });
  await opener.click();

  const dialog = page.getByRole("dialog", { name: "Your local-first engineering workspace" });
  await expect(dialog).toHaveAttribute("aria-modal", "true");
  await expect(dialog.getByText("AI operations cockpit")).toHaveCount(0);
  await expect(dialog.getByText("Code", { exact: true })).toBeVisible();
  await expect(dialog.getByText("Runs", { exact: true })).toBeVisible();
  await expect(dialog.getByText("Projects", { exact: true })).toBeVisible();

  await expect(dialog.getByRole("button", { name: "Get started" })).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(dialog.getByRole("button", { name: "Close" })).toBeFocused();

  await opener.focus();
  await page.keyboard.press("Tab");
  await expect(dialog.getByRole("button", { name: "Close" })).toBeFocused();

  await page.keyboard.press("Meta+k");
  await expect(page.getByRole("textbox", { name: "Search commands" })).toHaveCount(0);
  await expect(dialog).toBeVisible();

  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await expect(opener).toBeFocused();

  await openFromPalette(page, "Go to Dashboard");
  await expect(page.getByText("RepoDesk cockpit")).toHaveCount(0);
  await expect(page.getByRole("heading", { name: "Project state, context, and verification evidence." })).toBeVisible();
});

test("project switcher distinguishes registry failure from an empty registry", async ({ page }) => {
  await installMockIpc(page, {
    ...currentOnboardedFixtures,
    project_list_configs: {
      __mock_sequence: [
        { __mock_error: "registry unavailable" },
        { __mock_error: "registry unavailable" },
        currentOnboardedFixtures.project_list_configs,
      ],
    },
  });
  await page.goto("/");

  await page.getByRole("button", { name: "Show workspace sidebar" }).click();
  await page.locator(".workspace-sidebar").getByRole("button", { name: "RepoDesk", exact: true }).click();

  await expect(page.getByRole("alert")).toContainText("Could not load projects");
  await expect(page.getByRole("alert")).toContainText("registry unavailable");
  await page.getByRole("button", { name: "Retry loading projects" }).click();
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expect(page.locator(".project-switcher-menu").getByRole("button", { name: "RepoDesk", exact: true })).toBeVisible();
  await expect(page.getByText("No matching projects.")).toHaveCount(0);
});

test("artifact viewer clears stale identity across reopen and failure", async ({ page }) => {
  await installMockIpc(page, {
    ...currentOnboardedFixtures,
    read_artifact: {
      __mock_sequence: [
        {
          kind: "context",
          title: "First context artifact",
          path: "/tmp/first-context.md",
          exists: true,
          content: "first",
          size_bytes: 5,
        },
        {
          __mock_delay_ms: 250,
          __mock_value: {
            kind: "context",
            title: "Fresh context artifact",
            path: "/tmp/fresh-context.md",
            exists: true,
            content: "fresh",
            size_bytes: 5,
          },
        },
        { __mock_error: "artifact unavailable" },
      ],
    },
  });
  await page.goto("/");
  await page.evaluate(() => window.localStorage.setItem("repodesk.activeTab", "tokens"));
  await page.reload();
  await expect.poll(() => page.evaluate(() => window.localStorage.getItem("repodesk.activeTab"))).toBe("tokens");

  await expect(page.locator("main").getByText("Tokens", { exact: true }).first()).toBeVisible();
  const view = page.locator("main .table-row").filter({ hasText: "Context pack" }).getByRole("button", { name: "View" });
  await view.click();
  let dialog = page.getByRole("dialog", { name: "First context artifact" });
  await expect(dialog).toContainText("/tmp/first-context.md");
  await dialog.getByRole("button", { name: "Close" }).click();

  await view.click();
  dialog = page.getByRole("dialog", { name: "Artifact" });
  await expect(dialog).toContainText("Loading artifact content…");
  await expect(dialog).not.toContainText("/tmp/first-context.md");
  await expect(page.getByRole("dialog", { name: "Fresh context artifact" })).toContainText("/tmp/fresh-context.md");
  await page.getByRole("dialog", { name: "Fresh context artifact" }).getByRole("button", { name: "Close" }).click();

  await view.click();
  dialog = page.getByRole("dialog", { name: "Artifact" });
  await expect(dialog.getByRole("alert")).toContainText("artifact unavailable");
  await expect(dialog).not.toContainText("/tmp/fresh-context.md");
});

test("optional workspace tools request implementation assets only after first activation", async ({ page }) => {
  const resources: string[] = [];
  page.on("request", (request) => resources.push(request.url()));

  await installMockIpc(page, {
    ...currentOnboardedFixtures,
    action_history: [],
    terminal_create: {
      session_id: "terminal-trust-polish",
      cwd: "/Users/you/code/repodesk",
      pid: 4242,
      shell: "/bin/zsh",
    },
    terminal_resize: null,
  });
  await page.goto("/");
  await expect(page.getByRole("button", { name: "Command palette" })).toBeVisible();
  await page.waitForLoadState("networkidle");

  expect(matchingResources(resources, optionalResourceFragments)).toEqual([]);
  expect((await recordedCommands(page)).filter((command) => command === "terminal_create")).toHaveLength(0);

  await page.keyboard.press("Meta+k");
  await expect(page.getByRole("textbox", { name: "Search commands" })).toBeVisible();
  await expect.poll(() => matchingResources(resources, ["CommandPalette"]).length).toBeGreaterThan(0);
  await closePalette(page);

  const paletteRequests = matchingResources(resources, ["CommandPalette"]).length;
  await page.getByRole("button", { name: "Command palette" }).click();
  await expect(page.getByRole("textbox", { name: "Search commands" })).toBeVisible();
  await closePalette(page);
  await emitMockTauriEvent(page, "open-command-palette", null);
  await expect(page.getByRole("textbox", { name: "Search commands" })).toBeVisible();
  await closePalette(page);
  expect(matchingResources(resources, ["CommandPalette"])).toHaveLength(paletteRequests);

  await page.evaluate(() => {
    window.dispatchEvent(new CustomEvent("repodesk:bottom-panel-tab", { detail: "terminal" }));
  });
  const panel = page.getByRole("region", { name: "Workbench bottom panel" });
  await expect(panel).toBeVisible();
  await expect.poll(async () =>
    (await recordedCommands(page)).filter((command) => command === "terminal_create").length,
  ).toBe(1);
  await expect(panel.getByText("PID 4242")).toBeVisible();
  await expect.poll(() => matchingResources(resources, ["WorkbenchBottomPanel"]).length).toBeGreaterThan(0);
  await expect.poll(() => matchingResources(resources, ["InteractiveTerminal", "@xterm", "xterm.css"]).length).toBeGreaterThan(0);

  const panelRequests = matchingResources(resources, ["WorkbenchBottomPanel"]).length;
  const terminalRequests = matchingResources(resources, ["InteractiveTerminal", "@xterm", "xterm.css"]).length;
  await panel.getByRole("button", { name: "Close bottom panel" }).click();
  await page.keyboard.press("Meta+j");
  await expect(panel).toBeVisible();
  await panel.getByRole("button", { name: "Output", exact: false }).click();
  await panel.getByRole("button", { name: "Terminal", exact: true }).click();
  await page.getByRole("button", { name: /Code —/ }).click();
  await expect(page.getByRole("toolbar", { name: "Code workspace actions" })).toBeVisible();
  await page.getByRole("button", { name: /^Work —/ }).click();
  await expect(page.getByRole("region", { name: "Current Work Item" })).toBeVisible();
  await expect.poll(async () =>
    (await recordedCommands(page)).filter((command) => command === "terminal_create").length,
  ).toBe(1);
  expect(matchingResources(resources, ["WorkbenchBottomPanel"])).toHaveLength(panelRequests);
  expect(matchingResources(resources, ["InteractiveTerminal", "@xterm", "xterm.css"])).toHaveLength(terminalRequests);

  await panel.getByRole("button", { name: "Close bottom panel" }).click();
  await openFromPalette(page, "Toggle bottom panel");
  await expect(panel).toBeVisible();
  await panel.getByRole("button", { name: "Close bottom panel" }).click();
  await openFromPalette(page, "Run configured checks");
  await expect(panel).toBeVisible();
  await expect.poll(async () =>
    (await recordedCommands(page)).filter((command) => command === "terminal_create").length,
  ).toBe(1);

  const healthIndicator = page.getByRole("button", { name: /IDE health:/ });
  await healthIndicator.click();
  await expect(page.getByRole("dialog", { name: "IDE Health" })).toBeVisible();
  await expect.poll(() => matchingResources(resources, ["IDEHealthPanel.tsx", "health-panel.css"]).length).toBeGreaterThan(0);
});

test("persisted-open startup activates the bottom panel without creating a Terminal session", async ({ page }) => {
  const resources: string[] = [];
  const terminalResourceFragments = ["InteractiveTerminal", "vendor-terminal", "@xterm", "xterm.css"];
  page.on("request", (request) => resources.push(request.url()));

  await installMockIpc(page, {
    ...currentOnboardedFixtures,
    action_history: [],
    terminal_create: {
      session_id: "terminal-trust-polish",
      cwd: "/Users/you/code/repodesk",
      pid: 4242,
      shell: "/bin/zsh",
    },
    terminal_resize: null,
  });
  await page.addInitScript(() => {
    window.localStorage.setItem("repodesk.bottomPanelOpen", "1");
  });
  await page.goto("/");

  const panel = page.getByRole("region", { name: "Workbench bottom panel" });
  await expect(panel).toBeVisible();
  await page.waitForLoadState("networkidle");
  expect(matchingResources(resources, terminalResourceFragments)).toEqual([]);
  expect((await recordedCommands(page)).filter((command) => command === "terminal_create")).toHaveLength(0);
  await panel.getByRole("button", { name: "Terminal", exact: true }).click();
  await expect.poll(() => matchingResources(resources, terminalResourceFragments).length).toBeGreaterThan(0);
  await expect.poll(async () => (await recordedCommands(page)).filter((command) => command === "terminal_create").length).toBe(1);
  await expect(panel.getByText("PID 4242")).toBeVisible();

  await panel.getByRole("button", { name: "Close bottom panel" }).click();
  await page.getByRole("button", { name: "Show bottom panel" }).click();
  await panel.getByRole("button", { name: "Output", exact: false }).click();
  await panel.getByRole("button", { name: "Terminal", exact: true }).click();
  await expect.poll(async () => (await recordedCommands(page)).filter((command) => command === "terminal_create").length).toBe(1);
  await expect(panel.getByText("PID 4242")).toBeVisible();
});

test("a failed optional feature load is contained by its local error boundary", async ({ page }) => {
  await page.route("**/src/shared/ui/CommandPalette.tsx*", (route) => route.abort());
  await installMockIpc(page, currentOnboardedFixtures);
  await page.goto("/");

  const paletteButton = page.getByRole("button", { name: "Command palette" });
  await expect(paletteButton).toBeVisible();
  await paletteButton.click();

  const error = page.getByRole("alert");
  await expect(error).toContainText("This view crashed");
  await expect(error.getByRole("button", { name: "Try again" })).toBeVisible();
  await expect(page.getByText("RepoDesk hit an unexpected error")).toHaveCount(0);
  await expect(paletteButton).toBeVisible();
});

test("Orchestrate cleanup uses a RepoDesk decision instead of a browser dialog", async ({ page }) => {
  await installMockIpc(page, {
    ...currentOnboardedFixtures,
    orchestrate_worktrees: [
      {
        workspace_id: "wt-trust-polish",
        run_id: "run-trust-polish",
        step_id: "implement",
        path: "/tmp/repodesk/worktrees/wt-trust-polish",
        base_commit: "abc123",
        created_at: "2026-08-13T10:00:00Z",
        metadata_path: null,
        git_tracked: true,
        exists: true,
        dirty: true,
        changed_files: ["src/app.ts"],
        removable: true,
        warnings: [],
      },
    ],
    orchestrate_cleanup_worktree: {
      workspace_id: "wt-trust-polish",
      path: "/tmp/repodesk/worktrees/wt-trust-polish",
      removed: true,
      metadata_removed: true,
      warnings: [],
    },
  });
  let nativeDialogOpened = false;
  page.on("dialog", async (dialog) => {
    nativeDialogOpened = true;
    await dialog.dismiss();
  });
  await page.goto("/");
  await openFromPalette(page, "Go to Orchestrate");

  await page.getByRole("button", { name: "Cleanup worktree" }).click();
  const dialog = page.getByRole("dialog", { name: "Remove managed worktree?" });
  await expect(dialog).toContainText("1 changed file");
  expect(nativeDialogOpened).toBe(false);

  await dialog.getByRole("button", { name: "Remove worktree" }).click();
  await expect.poll(async () => (await recordedCommands(page)).filter((command) => command === "orchestrate_cleanup_worktree").length).toBe(1);
});
