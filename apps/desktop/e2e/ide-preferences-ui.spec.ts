import { expect, test, type Page } from "@playwright/test";
import { onboardedFixtures } from "./fixtures";
import { installMockIpc } from "./mock-ipc";

const settingsFixtures = {
  ...onboardedFixtures,
  provider_settings: {
    ollama_enabled: true,
    ollama_url: "http://127.0.0.1:11434",
    ollama_model: "qwen2.5-coder:7b",
    lm_studio_enabled: false,
    lm_studio_url: "http://127.0.0.1:1234",
    llamafile_enabled: false,
    llamafile_url: "http://127.0.0.1:8080",
    localai_enabled: false,
    localai_url: "http://127.0.0.1:8080",
    chatgpt_enabled: false,
    codex_enabled: true,
    gemini_enabled: false,
    openai_api_enabled: false,
    openai_api_key_env_var: "OPENAI_API_KEY",
    gemini_api_enabled: false,
    gemini_api_key_env_var: "GEMINI_API_KEY",
    anthropic_api_enabled: false,
    anthropic_api_key: "",
    openai_api_key: "",
    gemini_api_key: "",
    allow_paid_agents: false,
    codex_quota_status: "unknown",
    preferred_patch_provider: "codex",
    preferred_compression_provider: "ollama",
    preferred_review_provider: "codex",
    notes: "",
  },
};

async function openSettings(page: Page) {
  const sidebar = page.locator(".workspace-sidebar");
  if (!(await sidebar.isVisible())) {
    await page.getByRole("button", { name: "Show workspace sidebar" }).click();
  }
  await sidebar.getByRole("button", { name: "Settings", exact: true }).click();
}

test.describe("IDE preferences", () => {
  test("settings update CodeMirror and Explorer presentation", async ({ page }) => {
    await installMockIpc(page, settingsFixtures);
    await page.goto("/");

    await openSettings(page);
    await expect(page.getByRole("heading", { name: "Code workspace" })).toBeVisible();

    await page.getByRole("combobox", { name: "Editor font size" }).selectOption("16");
    await page.getByRole("combobox", { name: "Editor tab size" }).selectOption("4");
    await page.getByRole("combobox", { name: "Explorer density" }).selectOption("comfortable");
    await page.getByRole("checkbox", { name: "Word wrap" }).check();

    const stored = await page.evaluate(() => JSON.parse(localStorage.getItem("repodesk.ide-preferences.v1") || "{}"));
    expect(stored).toMatchObject({
      editorFontSize: 16,
      tabSize: 4,
      wordWrap: true,
      explorerDensity: "comfortable",
    });

    await page.getByRole("button", { name: /^Code —/ }).click();
    const row = page.getByRole("treeitem", { name: /.gitignore/ });
    await row.click();

    const editor = page.locator(".semantic-code-editor-host .cm-editor");
    const content = page.locator(".semantic-code-editor-host .cm-content");
    await expect(editor).toBeVisible();
    await expect(content).toHaveClass(/cm-lineWrapping/);

    const fontSize = await editor.evaluate((element) => getComputedStyle(element).fontSize);
    const tabSize = await content.evaluate((element) => getComputedStyle(element).tabSize);
    const rowBox = await row.boundingBox();

    expect(fontSize).toBe("16px");
    expect(tabSize).toBe("4");
    expect(Math.round(rowBox?.height ?? 0)).toBe(27);
  });
});
