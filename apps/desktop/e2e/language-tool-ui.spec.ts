import { expect, test } from "@playwright/test";
import { onboardedFixtures, type CommandFixtures } from "./fixtures";
import { installMockIpc, recordedInvocations } from "./mock-ipc";

const source = [
  "export function client(): number { return 1; }",
  "const value = client();",
].join("\n");

const capabilities = {
  diagnostics: true,
  hover: true,
  definition: true,
  references: true,
  completion: true,
  rename: true,
  formatting: true,
  document_symbols: true,
};

function server(overrides: Record<string, unknown> = {}) {
  return {
    id: "typescript-language-server",
    label: "TypeScript Language Server",
    executable: "typescript-language-server",
    arguments: ["--stdio"],
    languages: ["typescript", "javascript"],
    availability: "available",
    source: "path",
    profile_state: "active",
    initialization_profile: "default",
    install_recipe_id: "typescript-language-server",
    capabilities,
    ...overrides,
  };
}

function editorFixtures(serverFixture: ReturnType<typeof server>, syncDocument: unknown): CommandFixtures {
  return {
    ...onboardedFixtures,
    code_workspace_snapshot: {
      project: "RepoDesk",
      source: "git_index",
      truncated: false,
      files: [{
        path: "api.ts",
        name: "api.ts",
        extension: "ts",
        language: "typescript",
        bytes: source.length,
        status: "modified",
        blocked: false,
      }],
    },
    code_workspace_read: {
      path: "api.ts",
      content: source,
      bytes: source.length,
      line_count: 2,
      language: "typescript",
      status: "modified",
      fingerprint: "language-tool-ui",
    },
    "language_intelligence_snapshot:snapshot": {
      project: "RepoDesk",
      primary_language: "typescript",
      available_count: serverFixture.availability === "available" ? 1 : 0,
      generated_at: "2026-08-10T10:00:00Z",
      servers: [serverFixture],
    },
    "language_intelligence_snapshot:sync_document": syncDocument,
    "language_intelligence_snapshot:close_document": null,
    language_tool_install_status: null,
  };
}

const readyStatus = {
  project: "RepoDesk",
  server_id: "typescript-language-server",
  state: "ready",
  pid: 202,
  open_documents: 1,
  started_at: "2026-08-10T10:00:00Z",
  last_error: null,
};

const installPreview = {
  recipe_id: "typescript-language-server",
  recipe_revision: "typescript-language-server:5.3.0:typescript:7.0.2",
  server_id: "typescript-language-server",
  server_label: "TypeScript Language Server",
  languages: ["typescript", "javascript"],
  installer: "npm",
  package: "typescript-language-server",
  version: "5.3.0",
  destination: "/tmp/repodesk-dev/tools/language-servers/typescript-language-server",
  install_command: {
    program: "npm",
    args: [
      "install",
      "--prefix",
      "/tmp/repodesk-dev/tools/language-servers/.staging/typescript-language-server-fixture",
      "typescript-language-server@5.3.0",
      "typescript@7.0.2",
      "--ignore-scripts",
      "--no-audit",
      "--no-fund",
    ],
  },
  probe_command: {
    program: "/tmp/repodesk-dev/tools/language-servers/.staging/typescript-language-server-fixture/node_modules/.bin/typescript-language-server",
    args: ["--version"],
  },
  network_required: true,
  writes_outside_repository: ["/tmp/repodesk-dev/tools/language-servers"],
  prerequisite_available: true,
  prerequisite_hint: null,
  confirmation_token: "lang_install_fixture_token",
  expires_at: "2026-08-10T10:05:00Z",
};

async function openEditor(page: import("@playwright/test").Page, fixtures: CommandFixtures) {
  await installMockIpc(page, fixtures);
  await page.goto("/");
  await page.getByRole("button", { name: /^Code —/ }).click();
  await page.getByRole("treeitem", { name: /api.ts/ }).click();
  await expect(page.locator(".semantic-code-editor-host .cm-editor")).toBeVisible();
}

test.describe("language tool UI", () => {
  test("shows Ready state, capability details, and stays outside CodeMirror geometry", async ({ page }) => {
    await openEditor(page, editorFixtures(server(), readyStatus));

    const pill = page.getByRole("button", { name: "TypeScript language tool: Ready" });
    await expect(pill).toBeVisible();
    await pill.click();

    const popover = page.getByRole("dialog", { name: "TypeScript Language Server language tool" });
    await expect(popover).toContainText("System PATH");
    await expect(popover).toContainText("Definitions");
    await expect(popover).toContainText("References");

    const [pillBox, hostBox] = await Promise.all([
      pill.boundingBox(),
      page.locator(".semantic-code-editor-host").boundingBox(),
    ]);
    expect(pillBox).not.toBeNull();
    expect(hostBox).not.toBeNull();
    expect(pillBox!.y + pillBox!.height).toBeLessThanOrEqual(hostBox!.y);

    await pill.focus();
    await page.keyboard.press("Enter");
    await expect(popover).toHaveCount(0);
    await page.keyboard.press("Enter");
    await expect(popover).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(popover).toHaveCount(0);
  });

  test("shows Starting while the live server is initializing and then becomes Ready", async ({ page }) => {
    const fixtures = editorFixtures(server(), {
      __mock_delay_ms: 650,
      __mock_value: readyStatus,
    });
    await openEditor(page, fixtures);

    await expect(page.getByRole("button", { name: "TypeScript language tool: Starting" })).toBeVisible();
    await expect(page.getByRole("button", { name: "TypeScript language tool: Ready" })).toBeVisible({ timeout: 2_000 });
  });

  test("previews exact pinned installation, forwards only the token, and reports progress", async ({ page }) => {
    const fixtures = {
      ...editorFixtures(server({ availability: "missing", source: null }), null),
      language_tool_install_preview: installPreview,
      language_tool_install_confirm: {
        __mock_delay_ms: 800,
        __mock_value: {
          status: {
            recipe_id: "typescript-language-server",
            state: "ready",
            progress: 100,
            message: "TypeScript Language Server installed and verified",
            started_at: "2026-08-10T10:00:00Z",
            finished_at: "2026-08-10T10:00:01Z",
            error: null,
          },
          executable: "/tmp/repodesk-dev/tools/language-servers/typescript-language-server/node_modules/.bin/typescript-language-server",
          output: "installed",
        },
      },
    };
    await openEditor(page, fixtures);

    const pill = page.getByRole("button", { name: "TypeScript language tool: Missing" });
    await pill.click();
    await page.getByRole("button", { name: "Install", exact: true }).click();

    const dialog = page.getByRole("dialog", { name: "Install TypeScript Language Server" });
    await expect(dialog).toContainText("typescript-language-server@5.3.0");
    await expect(dialog).toContainText("typescript@7.0.2");
    await expect(dialog).toContainText("--ignore-scripts");
    await expect(dialog).toContainText("/tmp/repodesk-dev/tools/language-servers");
    await page.getByRole("button", { name: "Install language server" }).click();

    await expect(page.getByRole("button", { name: "TypeScript language tool: Installing" })).toBeVisible();
    await expect(page.locator(".language-tool-progress")).toContainText("5%");

    await expect.poll(async () => {
      const invocation = (await recordedInvocations(page))
        .find((call) => call.cmd === "language_tool_install_confirm");
      return invocation?.args;
    }).toEqual({ confirmationToken: "lang_install_fixture_token" });

    await expect(page.getByRole("button", { name: "TypeScript language tool: Ready" })).toBeVisible({ timeout: 2_000 });
  });

  test("can cancel an in-flight managed installation without inventing shell input", async ({ page }) => {
    const fixtures = {
      ...editorFixtures(server({ availability: "missing", source: null }), null),
      language_tool_install_preview: installPreview,
      language_tool_install_cancel: true,
      language_tool_install_confirm: {
        __mock_delay_ms: 1_500,
        __mock_value: {
          status: {
            recipe_id: "typescript-language-server",
            state: "cancelled",
            progress: 25,
            message: "Installation cancelled",
            started_at: "2026-08-10T10:00:00Z",
            finished_at: "2026-08-10T10:00:01Z",
            error: null,
          },
          executable: null,
          output: "cancelled",
        },
      },
    };
    await openEditor(page, fixtures);

    await page.getByRole("button", { name: "TypeScript language tool: Missing" }).click();
    await page.getByRole("button", { name: "Install", exact: true }).click();
    await page.getByRole("button", { name: "Install language server" }).click();
    await page.getByRole("button", { name: "Cancel installation" }).click();

    await expect.poll(async () => {
      const invocation = (await recordedInvocations(page))
        .find((call) => call.cmd === "language_tool_install_cancel");
      return invocation?.args;
    }).toEqual({ recipeId: "typescript-language-server" });
    await expect(page.getByRole("button", { name: "TypeScript language tool: Missing" })).toBeVisible();
  });

  test("shows installer errors with a bounded Retry path", async ({ page }) => {
    const fixtures = {
      ...editorFixtures(server({ availability: "missing", source: null }), null),
      language_tool_install_preview: installPreview,
      language_tool_install_confirm: {
        __mock_delay_ms: 300,
        __mock_value: {
          status: {
            recipe_id: "typescript-language-server",
            state: "error",
            progress: 40,
            message: "Version probe failed",
            started_at: "2026-08-10T10:00:00Z",
            finished_at: "2026-08-10T10:00:01Z",
            error: "Version probe failed",
          },
          executable: null,
          output: "probe failed",
        },
      },
    };
    await openEditor(page, fixtures);

    await page.getByRole("button", { name: "TypeScript language tool: Missing" }).click();
    await page.getByRole("button", { name: "Install", exact: true }).click();
    await page.getByRole("button", { name: "Install language server" }).click();

    await expect(page.getByRole("button", { name: "TypeScript language tool: Error" })).toBeVisible({ timeout: 1_500 });
    await expect(page.getByRole("dialog", { name: "TypeScript Language Server language tool" })).toContainText("Version probe failed");
    await page.getByRole("button", { name: "Retry" }).click();
    await expect(page.getByRole("dialog", { name: "Install TypeScript Language Server" })).toBeVisible();
  });

  test("labels discovery-only profiles honestly and offers no fake start/install action", async ({ page }) => {
    const fixtures = editorFixtures(server({
      id: "pyright",
      label: "Pyright",
      executable: "pyright-langserver",
      languages: ["typescript"],
      profile_state: "discovery_only",
      install_recipe_id: null,
    }), null);
    await openEditor(page, fixtures);

    const pill = page.getByRole("button", { name: "TypeScript language tool: Discovery only" });
    await pill.click();
    const popover = page.getByRole("dialog", { name: "Pyright language tool" });
    await expect(popover).toContainText("live support is not enabled");
    await expect(popover.getByRole("button", { name: /Install|Retry|Start/ })).toHaveCount(0);
  });
});
