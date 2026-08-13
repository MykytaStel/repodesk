import { expect, test } from "@playwright/test";
import { onboardedFixtures } from "./fixtures";
import { installMockIpc, recordedInvocations } from "./mock-ipc";

const quickOpenPath = "src/important/session_manager.rs";
const quickOpenSource = "export function restoreSession() { return true; }\n";

const quickOpenFixtures = {
  ...onboardedFixtures,
  code_workspace_quick_open: [
    {
      path: quickOpenPath,
      name: "session_manager.rs",
      language: "rust",
      status: "modified",
    },
  ],
  code_workspace_snapshot: {
    project: "RepoDesk",
    source: "git_index",
    truncated: false,
    files: [
      {
        path: quickOpenPath,
        name: "session_manager.rs",
        extension: "rs",
        language: "rust",
        bytes: quickOpenSource.length,
        status: "modified",
        blocked: false,
      },
    ],
  },
  code_workspace_read: {
    path: quickOpenPath,
    content: quickOpenSource,
    bytes: quickOpenSource.length,
    line_count: 1,
    language: "rust",
    status: "modified",
    fingerprint: "quick-open-session-manager",
  },
  "language_intelligence_snapshot:snapshot": {
    project: "RepoDesk",
    primary_language: "rust",
    available_count: 0,
    generated_at: "2026-08-13T00:00:00Z",
    servers: [],
  },
};

test.describe("Whole-repository Quick Open", () => {
  test("searches through backend index without preloading the Code workspace", async ({ page }) => {
    await installMockIpc(page, quickOpenFixtures);
    await page.goto("/");

    // The shortcut contract begins once the application shell is interactive;
    // pressing a browser key while React is still booting tests navigation timing,
    // not RepoDesk Quick Open. Keep this in sync with the daily-loop shortcut test.
    await expect(page.locator(".phase-rail")).toBeVisible();
    await page.locator("body").click();
    await page.keyboard.press("ControlOrMeta+k");
    const input = page.getByRole("textbox", { name: "Search commands" });
    await expect(input).toBeFocused();
    await input.fill("session");

    const result = page.getByRole("button", { name: /Open file: session_manager.rs/ });
    await expect(result).toBeVisible();

    const beforeOpen = await recordedInvocations(page);
    expect(beforeOpen).toContainEqual({
      cmd: "code_workspace_quick_open",
      args: { query: "session", limit: 50 },
    });
    expect(beforeOpen.some(({ cmd }) => cmd === "code_workspace_snapshot")).toBe(false);

    await result.click();

    await expect(page.locator(".code-document-location")).toContainText(quickOpenPath);
    await expect(page.locator(".semantic-code-editor-host .cm-content")).toContainText("restoreSession");

    const afterOpen = await recordedInvocations(page);
    expect(afterOpen.some(({ cmd }) => cmd === "code_workspace_snapshot")).toBe(true);
    expect(afterOpen).toContainEqual({
      cmd: "code_workspace_read",
      args: { relativePath: quickOpenPath },
    });
  });
});
