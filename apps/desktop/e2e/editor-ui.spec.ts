import { expect, test } from "@playwright/test";
import { onboardedFixtures } from "./fixtures";
import { installMockIpc, recordedInvocations } from "./mock-ipc";

const rustSource = [
  "fn health_trend() -> u32 { 42 }",
  "fn main() { let value = health_trend(); }",
].join("\n");

const rustEditorFixtures = {
  ...onboardedFixtures,
  code_workspace_snapshot: {
    project: "RepoDesk",
    source: "git_index",
    truncated: false,
    files: [{
      path: "src/main.rs",
      name: "main.rs",
      extension: "rs",
      language: "rust",
      bytes: rustSource.length,
      status: "modified",
      blocked: false,
    }],
  },
  code_workspace_read: {
    path: "src/main.rs",
    content: rustSource,
    bytes: rustSource.length,
    line_count: 2,
    language: "rust",
    status: "modified",
    fingerprint: "rust-editor-ui-fixture",
  },
  "language_intelligence_snapshot:snapshot": {
    project: "RepoDesk",
    primary_language: "rust",
    available_count: 1,
    generated_at: "2026-08-09T20:00:00Z",
    servers: [{
      id: "rust-analyzer",
      label: "rust-analyzer",
      executable: "rust-analyzer",
      arguments: [],
      languages: ["rust"],
      availability: "available",
      source: "path",
      profile_state: "active",
      initialization_profile: "default",
      install_recipe_id: "rust-analyzer",
      capabilities: {
        diagnostics: true,
        hover: true,
        definition: true,
        references: true,
        completion: true,
        rename: true,
        formatting: true,
        document_symbols: true,
      },
    }],
  },
  "language_intelligence_snapshot:sync_document": {
    project: "RepoDesk",
    server_id: "rust-analyzer",
    state: "ready",
    pid: 101,
    open_documents: 1,
    started_at: "2026-08-09T20:00:00Z",
    last_error: null,
  },
  "language_intelligence_snapshot:hover": {
    markdown: "(alias) fn health_trend() -> u32\n\nReturns the current health trend.",
    range: {
      start: { line: 1, character: 24 },
      end: { line: 1, character: 36 },
    },
  },
  "language_intelligence_snapshot:definition": [{
    path: "src/main.rs",
    line: 1,
    column: 4,
    end_line: 1,
    end_column: 16,
  }],
  "language_intelligence_snapshot:close_document": null,
};

function languageEditorFixtures({
  path,
  language,
  source,
  serverId,
  serverLabel,
  capabilities,
  hover = null,
  definition = [],
}: {
  path: string;
  language: string;
  source: string;
  serverId: string;
  serverLabel: string;
  capabilities: typeof rustEditorFixtures["language_intelligence_snapshot"]["servers"][number]["capabilities"];
  hover?: unknown;
  definition?: unknown[];
}) {
  const name = path.split("/").at(-1) ?? path;
  const extension = name.includes(".") ? name.split(".").at(-1) ?? "" : "";
  return {
    ...onboardedFixtures,
    code_workspace_snapshot: {
      project: "RepoDesk",
      source: "git_index",
      truncated: false,
      files: [{
        path,
        name,
        extension,
        language,
        bytes: source.length,
        status: "modified",
        blocked: false,
      }],
    },
    code_workspace_read: {
      path,
      content: source,
      bytes: source.length,
      line_count: source.split("\n").length,
      language,
      status: "modified",
      fingerprint: `${language}-editor-ui-fixture`,
    },
    "language_intelligence_snapshot:snapshot": {
      project: "RepoDesk",
      primary_language: language,
      available_count: 1,
      generated_at: "2026-08-10T08:00:00Z",
      servers: [{
        id: serverId,
        label: serverLabel,
        executable: serverId,
        arguments: [],
        languages: language === "typescript" ? ["typescript", "javascript"] : [language],
        availability: "available",
        source: "path",
        profile_state: "active",
        initialization_profile: serverId === "taplo" ? "taplo" : "default",
        install_recipe_id: serverId,
        capabilities,
      }],
    },
    "language_intelligence_snapshot:sync_document": {
      project: "RepoDesk",
      server_id: serverId,
      state: "ready",
      pid: 202,
      open_documents: 1,
      started_at: "2026-08-10T08:00:00Z",
      last_error: null,
    },
    "language_intelligence_snapshot:hover": hover,
    "language_intelligence_snapshot:definition": definition,
    "language_intelligence_snapshot:close_document": null,
  };
}

const fullLanguageCapabilities = {
  diagnostics: true,
  hover: true,
  definition: true,
  references: true,
  completion: true,
  rename: true,
  formatting: true,
  document_symbols: true,
};

const typescriptSource = [
  "function client(): number { return 1; }",
  "const value = client();",
].join("\n");

const typescriptEditorFixtures = languageEditorFixtures({
  path: "api.ts",
  language: "typescript",
  source: typescriptSource,
  serverId: "typescript-language-server",
  serverLabel: "TypeScript Language Server",
  capabilities: fullLanguageCapabilities,
  hover: {
    markdown: "function client(): number",
    range: {
      start: { line: 1, character: 14 },
      end: { line: 1, character: 20 },
    },
  },
  definition: [{
    path: "api.ts",
    line: 1,
    column: 10,
    end_line: 1,
    end_column: 16,
  }],
});

const tomlEditorFixtures = languageEditorFixtures({
  path: "Cargo.toml",
  language: "toml",
  source: "[package]\nname = \"repodesk\"",
  serverId: "taplo",
  serverLabel: "Taplo",
  capabilities: fullLanguageCapabilities,
});

const metadataOnlyCapabilities = {
  diagnostics: true,
  hover: false,
  definition: false,
  references: false,
  completion: true,
  rename: false,
  formatting: true,
  document_symbols: true,
};

async function textCenter(page: import("@playwright/test").Page, lineIndex: number, text: string) {
  return page.locator(".semantic-code-editor-host .cm-line").nth(lineIndex).evaluate((content, needle) => {
    const walker = document.createTreeWalker(content, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) {
      const node = walker.currentNode;
      const value = node.textContent ?? "";
      const index = value.lastIndexOf(needle);
      if (index < 0) continue;
      const range = document.createRange();
      range.setStart(node, index);
      range.setEnd(node, index + needle.length);
      const box = range.getBoundingClientRect();
      return { x: box.left + box.width / 2, y: box.top + box.height / 2 };
    }
    throw new Error(`Text not found: ${needle}`);
  }, text);
}

async function requestNavigationTarget(page: import("@playwright/test").Page) {
  await page.evaluate(() => {
    window.sessionStorage.setItem("repodesk.code.location-request", JSON.stringify({
      path: ".gitignore",
      line: 13,
      column: 5,
      endLine: 13,
      endColumn: 11,
    }));
    window.dispatchEvent(new CustomEvent("repodesk:open-code", { detail: { path: ".gitignore" } }));
  });
}

test.describe("semantic code editor geometry", () => {
  test.beforeEach(async ({ page }) => {
    await installMockIpc(page, onboardedFixtures);
    await page.goto("/");
    await page.getByRole("button", { name: /^Code —/ }).click();
    await page.getByRole("treeitem", { name: /.gitignore/ }).click();
    await expect(page.locator(".semantic-code-editor-host .cm-editor")).toBeVisible();
  });

  test("CodeMirror owns scrolling and renders one active line number", async ({ page }) => {
    const scroller = page.locator(".semantic-code-editor-host .cm-scroller");
    const lineNumbers = page.locator(".semantic-code-editor-host .cm-lineNumbers");

    await expect(lineNumbers.locator(".cm-activeLineGutter")).toHaveCount(1);
    expect(await lineNumbers.locator(".cm-gutterElement").count()).toBeGreaterThan(20);
    expect(await scroller.evaluate((element) => element.scrollHeight > element.clientHeight)).toBe(true);
  });

  test("the CodeMirror gutter stays fixed during two-axis scrolling", async ({ page }) => {
    const scroller = page.locator(".semantic-code-editor-host .cm-scroller");
    const gutters = page.locator(".semantic-code-editor-host .cm-gutters");
    const before = await gutters.boundingBox();

    await scroller.evaluate((element) => {
      element.scrollTop = 600;
      element.scrollLeft = 420;
      element.dispatchEvent(new Event("scroll", { bubbles: true }));
    });

    await expect.poll(() => scroller.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
    const after = await gutters.boundingBox();
    expect(before).not.toBeNull();
    expect(after).not.toBeNull();
    expect(Math.abs(after!.x - before!.x)).toBeLessThanOrEqual(1);
  });

  test("the only editor scrollbar belongs to the right-side scroller", async ({ page }) => {
    const editor = page.locator(".semantic-code-editor-host .cm-editor");
    const scroller = page.locator(".semantic-code-editor-host .cm-scroller");
    const gutters = page.locator(".semantic-code-editor-host .cm-gutters");
    const [editorBox, gutterBox] = await Promise.all([editor.boundingBox(), gutters.boundingBox()]);

    expect(editorBox).not.toBeNull();
    expect(gutterBox).not.toBeNull();
    expect(gutterBox!.x + gutterBox!.width).toBeLessThan(editorBox!.x + editorBox!.width);
    expect(await scroller.evaluate((element) =>
      getComputedStyle(element, "::-webkit-scrollbar").width,
    )).toBe("10px");
    expect(await gutters.evaluate((element) => ({
      vertical: element.scrollHeight === element.clientHeight,
      horizontal: element.scrollWidth === element.clientWidth,
    }))).toEqual({ vertical: true, horizontal: true });
  });

  test("clicking a CodeMirror line number moves the caret and fills its gutter", async ({ page }) => {
    const content = page.locator(".semantic-code-editor-host .cm-content");
    const lineNumbers = page.locator(".semantic-code-editor-host .cm-lineNumbers");
    const line13 = lineNumbers.locator(".cm-gutterElement", { hasText: /^13$/ });

    await line13.click();

    await expect(content).toBeFocused();
    await expect(page.locator(".code-editor-status")).toContainText("Ln 13, Col 1");
    const active = lineNumbers.locator(".cm-activeLineGutter");
    await expect(active).toHaveText("13");
    expect(await active.evaluate((element) => getComputedStyle(element).backgroundColor)).not.toBe("rgba(0, 0, 0, 0)");

    const [gutterBox, activeBox] = await Promise.all([lineNumbers.boundingBox(), active.boundingBox()]);
    expect(gutterBox).not.toBeNull();
    expect(activeBox).not.toBeNull();
    expect(Math.abs(activeBox!.x - gutterBox!.x)).toBeLessThanOrEqual(1);
    expect(Math.abs(
      activeBox!.x + activeBox!.width - (gutterBox!.x + gutterBox!.width),
    )).toBeLessThanOrEqual(1);
  });

  test("reveals the exact navigation target without turning it into a selection", async ({ page }) => {
    await requestNavigationTarget(page);

    const target = page.locator(".semantic-code-editor-host .cm-navigation-target");
    const targetLine = page.locator(".semantic-code-editor-host .cm-navigation-target-line");
    await expect(target).toHaveText("# sour");
    await expect(targetLine).toHaveCount(1);
    await expect(page.locator(".code-editor-status")).toContainText("Ln 13, Col 5");
    expect(await page.locator(".semantic-code-editor-host .cm-editor").evaluate((editor) => {
      const selection = window.getSelection();
      return {
        selectionText: selection?.toString() ?? "",
        targetLineCount: editor.querySelectorAll(".cm-navigation-target-line").length,
      };
    })).toEqual({ selectionText: "", targetLineCount: 1 });
  });

  test("clears the navigation target when editing starts", async ({ page }) => {
    await requestNavigationTarget(page);
    const target = page.locator(".semantic-code-editor-host .cm-navigation-target");
    await expect(target).toBeVisible();

    await page.locator(".semantic-code-editor-host .cm-content").press("x");

    await expect(target).toHaveCount(0);
    await expect(page.locator(".cm-navigation-target-line")).toHaveCount(0);
  });

  test("clears the navigation target after the reveal interval", async ({ page }) => {
    await requestNavigationTarget(page);
    const target = page.locator(".semantic-code-editor-host .cm-navigation-target");
    await expect(target).toBeVisible();

    await expect(target).toHaveCount(0, { timeout: 2_500 });
    await expect(page.locator(".cm-navigation-target-line")).toHaveCount(0);
    await expect(page.locator(".code-editor-status")).toContainText("Ln 13, Col 5");
  });
});

test.describe("semantic definition navigation", () => {
  test.beforeEach(async ({ page }) => {
    await installMockIpc(page, rustEditorFixtures);
    await page.goto("/");
    await page.getByRole("button", { name: /^Code —/ }).click();
    await page.getByRole("treeitem", { name: /^src 1$/ }).click();
    await page.getByRole("treeitem", { name: /main.rs/ }).click();
    await expect(page.locator(".semantic-code-editor-host .cm-editor")).toBeVisible();
    await expect(page.locator(".code-language-service")).toContainText("RA ready");
  });

  test("modifier-hover links a resolvable symbol and previews it without moving the caret", async ({ page }) => {
    const before = await page.locator(".code-editor-status").innerText();
    const point = await textCenter(page, 1, "health_trend");

    await page.keyboard.down("Control");
    await page.mouse.move(point.x, point.y);

    await expect(page.locator(".cm-definition-link")).toHaveText("health_trend");
    await expect(page.locator(".cm-definition-preview")).toContainText("(alias) fn health_trend() -> u32");
    expect(await page.locator(".code-editor-status").innerText()).toBe(before);

    await expect.poll(async () => {
      const invocations = await recordedInvocations(page);
      const previewCalls = invocations.filter((call) => {
        const action = call.args?.action as { kind?: string } | undefined;
        return call.cmd === "language_intelligence_snapshot"
          && (action?.kind === "hover" || action?.kind === "definition");
      });
      return previewCalls.map((call) => {
        const action = call.args?.action as { kind: string; line: number; column: number };
        return { kind: action.kind, line: action.line, column: action.column };
      });
    }).toEqual([
      { kind: "hover", line: 2, column: 31 },
      { kind: "definition", line: 2, column: 31 },
    ]);

    await page.keyboard.up("Control");
    await expect(page.locator(".cm-definition-link")).toHaveCount(0);
    await expect(page.locator(".cm-definition-preview")).toHaveCount(0);
  });

  test("a normal click does not request language navigation", async ({ page }) => {
    const point = await textCenter(page, 1, "health_trend");

    await page.mouse.click(point.x, point.y);

    const invocations = await recordedInvocations(page);
    const navigationCalls = invocations.filter((call) => {
      const action = call.args?.action as { kind?: string } | undefined;
      return call.cmd === "language_intelligence_snapshot"
        && (action?.kind === "hover" || action?.kind === "definition");
    });
    expect(navigationCalls).toEqual([]);
    await expect(page.locator(".cm-definition-link")).toHaveCount(0);
    await expect(page.locator(".cm-definition-preview")).toHaveCount(0);
  });

  test("modifier-click reveals the exact definition range", async ({ page }) => {
    const point = await textCenter(page, 1, "health_trend");

    await page.keyboard.down("Control");
    await page.mouse.click(point.x, point.y);
    await page.keyboard.up("Control");

    await expect.poll(async () => {
      const invocations = await recordedInvocations(page);
      return invocations.filter((call) => {
        const action = call.args?.action as { kind?: string } | undefined;
        return call.cmd === "language_intelligence_snapshot" && action?.kind === "definition";
      }).length;
    }).toBe(1);
    await expect.poll(() => page.evaluate(() => (
      window.sessionStorage.getItem("repodesk.code.location-request")
    ))).toBeNull();
    await expect(page.locator(".cm-navigation-target")).toHaveText("health_trend");
    await expect(page.locator(".cm-navigation-target-line")).toHaveCount(1);
    await expect(page.locator(".code-editor-status")).toContainText("Ln 1, Col 4");
  });
});

test.describe("TypeScript intelligence", () => {
  test.beforeEach(async ({ page }) => {
    await installMockIpc(page, typescriptEditorFixtures);
    await page.goto("/");
    await page.getByRole("button", { name: /^Code —/ }).click();
    await page.getByRole("treeitem", { name: /api.ts/ }).click();
    await expect(page.locator(".semantic-code-editor-host .cm-editor")).toBeVisible();
    await expect(page.locator(".code-language-service")).toContainText("TS ready");
  });

  test("modifier-hover previews a definition and F12 reveals its exact range", async ({ page }) => {
    const point = await textCenter(page, 1, "client");

    await page.keyboard.down("Control");
    await page.mouse.move(point.x, point.y);

    await expect(page.locator(".cm-definition-link")).toHaveText("client");
    await expect(page.locator(".cm-definition-preview")).toContainText("function client(): number");

    await page.keyboard.up("Control");
    await page.mouse.click(point.x, point.y);
    await page.keyboard.press("F12");

    await expect(page.locator(".cm-navigation-target")).toHaveText("client");
    await expect(page.locator(".code-editor-status")).toContainText("Ln 1, Col 10");
  });

  test("synchronizes the document with its TypeScript language id", async ({ page }) => {
    await expect.poll(async () => {
      const invocations = await recordedInvocations(page);
      const sync = invocations.find((call) => {
        const action = call.args?.action as { kind?: string } | undefined;
        return call.cmd === "language_intelligence_snapshot" && action?.kind === "sync_document";
      });
      return (sync?.args?.action as { language?: string } | undefined)?.language;
    }).toBe("typescript");
  });
});

test.describe("TOML intelligence", () => {
  test("starts Taplo and synchronizes TOML instead of falling back to discovery-only UI", async ({ page }) => {
    await installMockIpc(page, tomlEditorFixtures);
    await page.goto("/");
    await page.getByRole("button", { name: /^Code —/ }).click();
    await page.getByRole("treeitem", { name: /Cargo.toml/ }).click();

    await expect(page.locator(".code-language-service")).toContainText("TOML ready");
    await expect.poll(async () => {
      const invocations = await recordedInvocations(page);
      const sync = invocations.find((call) => {
        const action = call.args?.action as { kind?: string } | undefined;
        return call.cmd === "language_intelligence_snapshot" && action?.kind === "sync_document";
      });
      return (sync?.args?.action as { language?: string } | undefined)?.language;
    }).toBe("toml");
  });
});

for (const language of ["json", "yaml"] as const) {
  test(`${language.toUpperCase()} intelligence respects disabled navigation capabilities`, async ({ page }) => {
    const extension = language === "json" ? "json" : "yaml";
    const serverId = language === "json" ? "json-language-server" : "yaml-language-server";
    await installMockIpc(page, languageEditorFixtures({
      path: `config.${extension}`,
      language,
      source: language === "json" ? "{\"enabled\": true}" : "enabled: true",
      serverId,
      serverLabel: language === "json" ? "JSON Language Server" : "YAML Language Server",
      capabilities: metadataOnlyCapabilities,
    }));
    await page.goto("/");
    await page.getByRole("button", { name: /^Code —/ }).click();
    await page.getByRole("treeitem", { name: new RegExp(`config\\.${extension}`) }).click();

    await expect(page.locator(".code-language-service")).toContainText(`${language.toUpperCase()} ready`);
    const point = await textCenter(page, 0, "enabled");
    await page.keyboard.down("Control");
    await page.mouse.move(point.x, point.y);
    await page.waitForTimeout(250);
    await page.keyboard.up("Control");

    const invocations = await recordedInvocations(page);
    const navigationCalls = invocations.filter((call) => {
      const action = call.args?.action as { kind?: string } | undefined;
      return call.cmd === "language_intelligence_snapshot"
        && (action?.kind === "hover" || action?.kind === "definition");
    });
    expect(navigationCalls).toEqual([]);
  });
}
