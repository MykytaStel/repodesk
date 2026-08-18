import { expect, test, type Page } from "@playwright/test";
import { currentOnboardedFixtures } from "./current-fixtures";
import { installMockIpc, recordedCommands, recordedInvocations } from "./mock-ipc";

const providerPreferences = {
  ollama_enabled: true,
  ollama_url: "http://127.0.0.1:11434",
  ollama_model: "qwen2.5-coder:7b",
  lm_studio_enabled: false,
  lm_studio_url: "http://127.0.0.1:1234",
  llamafile_enabled: false,
  llamafile_url: "http://127.0.0.1:8080",
  localai_enabled: false,
  localai_url: "http://127.0.0.1:8080",
  chatgpt_enabled: true,
  codex_enabled: true,
  gemini_enabled: false,
  openai_api_enabled: true,
  openai_api_key_env_var: "OPENAI_API_KEY",
  gemini_api_enabled: false,
  gemini_api_key_env_var: "GEMINI_API_KEY",
  anthropic_api_enabled: false,
  allow_paid_agents: true,
  codex_quota_status: "available",
  preferred_patch_provider: "codex_cli",
  preferred_compression_provider: "ollama",
  preferred_review_provider: "codex_cli",
  notes: "",
};

const emptyCredential = (key: string) => ({ key, configured: false, hint: "", source: "none" });

async function openSettings(page: Page) {
  const sidebar = page.locator(".workspace-sidebar");
  if (!(await sidebar.isVisible())) {
    await page.getByRole("button", { name: "Show Navigator" }).click();
  }
  await sidebar.getByRole("button", { name: "Settings", exact: true }).click();
}

test.describe("Settings credential ownership", () => {
  test("environment credential has one editor, no delete, and can be overridden in keychain", async ({ page }) => {
    const fixtures = {
      ...currentOnboardedFixtures,
      provider_preferences: providerPreferences,
      credential_status: {
        __mock_sequence: [
          [
            { key: "openai_api_key", configured: true, hint: "••••env1", source: "environment" },
            emptyCredential("anthropic_api_key"),
            emptyCredential("gemini_api_key"),
          ],
          [
            { key: "openai_api_key", configured: true, hint: "••••new1", source: "keychain" },
            emptyCredential("anthropic_api_key"),
            emptyCredential("gemini_api_key"),
          ],
        ],
      },
      credential_set: { key: "openai_api_key", configured: true, hint: "••••new1", source: "keychain" },
    };

    await installMockIpc(page, fixtures);
    await page.goto("/");
    await openSettings(page);

    await expect(page.getByRole("heading", { name: "Credentials", exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Save API keys" })).toHaveCount(0);
    await expect(page.getByText("Environment · ••••env1", { exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Delete OpenAI API key" })).toHaveCount(0);

    const openaiInput = page.locator("#credential-openai_api_key");
    await openaiInput.fill("fixture-cccc");
    await page.getByRole("button", { name: "Save OpenAI API key" }).click();

    await expect(openaiInput).toHaveValue("");
    await expect(page.getByText("Keychain · ••••new1", { exact: true })).toBeVisible();

    const invocations = await recordedInvocations(page);
    expect(invocations).toContainEqual({
      cmd: "credential_set",
      args: { key: "openai_api_key", value: "fixture-cccc" },
    });

    const commands = await recordedCommands(page);
    expect(commands.filter((command) => command === "credential_status").length).toBeGreaterThanOrEqual(2);
    expect(commands.filter((command) => command === "model_health_snapshot").length).toBeGreaterThanOrEqual(2);
    expect(commands.filter((command) => command === "get_api_env_diagnostic").length).toBeGreaterThanOrEqual(2);
  });

  test("deleting a keychain override reveals the read-only environment fallback", async ({ page }) => {
    const fixtures = {
      ...currentOnboardedFixtures,
      provider_preferences: providerPreferences,
      credential_status: {
        __mock_sequence: [
          [
            { key: "openai_api_key", configured: true, hint: "••••key1", source: "keychain" },
            emptyCredential("anthropic_api_key"),
            emptyCredential("gemini_api_key"),
          ],
          [
            { key: "openai_api_key", configured: true, hint: "••••env2", source: "environment" },
            emptyCredential("anthropic_api_key"),
            emptyCredential("gemini_api_key"),
          ],
        ],
      },
      credential_delete: { key: "openai_api_key", configured: true, hint: "••••env2", source: "environment" },
    };

    await installMockIpc(page, fixtures);
    await page.goto("/");
    await openSettings(page);

    await expect(page.getByText("Keychain · ••••key1", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Delete OpenAI API key" }).click();
    await expect(page.getByText("Environment · ••••env2", { exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Delete OpenAI API key" })).toHaveCount(0);

    const invocations = await recordedInvocations(page);
    expect(invocations).toContainEqual({
      cmd: "credential_delete",
      args: { key: "openai_api_key" },
    });
  });

  test("credential status failure never masquerades as not configured", async ({ page }) => {
    const fixtures = {
      ...currentOnboardedFixtures,
      provider_preferences: providerPreferences,
      credential_status: { __mock_error: "secure store unavailable" },
    };

    await installMockIpc(page, fixtures);
    await page.goto("/");
    await openSettings(page);

    await expect(page.getByRole("alert")).toContainText("Credential status is unavailable");
    await expect(page.getByText("Status unavailable", { exact: true })).toHaveCount(3);
    await expect(page.getByText("Not configured", { exact: true })).toHaveCount(0);
    await expect(page.getByRole("button", { name: /^Delete .* API key$/ })).toHaveCount(0);
  });
});
