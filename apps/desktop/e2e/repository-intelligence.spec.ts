import { expect, test } from "@playwright/test";
import { onboardedFixtures } from "./fixtures";
import { installMockIpc, recordedInvocations } from "./mock-ipc";

const source = [
  'import { client } from "./api";',
  "export const value = client();",
].join("\n");

const repositoryIntelligenceFixtures = {
  ...onboardedFixtures,
  code_workspace_snapshot: {
    project: "RepoDesk",
    source: "git_index",
    truncated: false,
    files: [{
      path: "src/app.ts",
      name: "app.ts",
      extension: "ts",
      language: "typescript",
      bytes: source.length,
      status: "modified",
      blocked: false,
    }],
  },
  code_workspace_read: {
    path: "src/app.ts",
    content: source,
    bytes: source.length,
    line_count: 2,
    language: "typescript",
    status: "modified",
    fingerprint: "repo-intel-fixture",
  },
  repository_intelligence_snapshot: {
    version: 3,
    project: "RepoDesk",
    focus_path: "src/app.ts",
    indexed_files: 7,
    rust_files_indexed: 1,
    rust_bytes_indexed: 240,
    coverage: {
      semantic_files_eligible: 4,
      semantic_files_indexed: 3,
      semantic_bytes_indexed: 760,
      languages: [
        {
          language: "typescript",
          visible_files: 3,
          semantic_files_indexed: 2,
          semantic_bytes_indexed: 520,
          strategy: "script_literal_imports",
          evidence_level: "bounded",
          truncated: false,
          limitations: [
            "Only relative literal imports are resolved; package imports, aliases, and computed imports remain unknown.",
          ],
        },
        {
          language: "rust",
          visible_files: 1,
          semantic_files_indexed: 1,
          semantic_bytes_indexed: 240,
          strategy: "rust_ast",
          evidence_level: "bounded",
          truncated: true,
          limitations: [
            "Only local Rust module/use relationships are resolved; macros and external crates are not expanded.",
            "The Rust AST semantic index hit its bound, so Rust reverse edges and coverage may be incomplete.",
          ],
        },
        {
          language: "html",
          visible_files: 3,
          semantic_files_indexed: 0,
          semantic_bytes_indexed: 0,
          strategy: "unavailable",
          evidence_level: "unavailable",
          truncated: false,
          limitations: [
            "Dependency/dependent lists can be empty even when real relationships exist.",
          ],
        },
      ],
    },
    truncated: true,
    git_history_available: true,
    focus: {
      path: "src/app.ts",
      language: "typescript",
      graph_evidence: {
        strategy: "script_literal_imports",
        level: "bounded",
        indexed: true,
        reasons: [
          "The focus file was scanned for local literal TypeScript/JavaScript import evidence.",
        ],
        limitations: [
          "Only relative literal imports are resolved; package imports, aliases, and computed imports remain unknown.",
        ],
      },
      dependencies: [{ path: "src/api.ts", reason: "import ./api" }],
      dependents: [{ path: "src/main.ts", reason: "referenced by src/main.ts" }],
      closest_tests: [{ path: "src/app.test.ts", score: 100, reason: "test file directly depends on focus" }],
      co_changes: [{ path: "src/state.ts", commits_together: 3, focus_commits_sampled: 5 }],
      context_candidates: [{
        path: "src/api.ts",
        score: 92,
        reasons: ["dependency: import ./api"],
      }],
    },
  },
};

test("Repository intelligence explains semantic coverage and graph confidence", async ({ page }) => {
  await installMockIpc(page, repositoryIntelligenceFixtures);
  await page.goto("/");
  await page.getByRole("button", { name: /^Code —/ }).click();
  await page.getByRole("treeitem", { name: /src/ }).click();
  await page.getByRole("treeitem", { name: /app\.ts/ }).click();
  await expect(page.locator(".semantic-code-editor-host .cm-editor")).toBeVisible();

  await page.getByRole("button", { name: "Repo context" }).click();

  const drawer = page.getByRole("complementary", { name: "Repository intelligence" });
  await expect(drawer).toBeVisible();
  await expect(drawer.locator(".repo-intel-meta")).toContainText("3/4 semantic files indexed");
  await expect(drawer.locator(".repo-intel-meta")).toContainText("bounded index");

  const evidence = drawer.locator(".repo-intel-evidence");
  await expect(evidence).toContainText("Graph evidence");
  await expect(evidence).toContainText("Bounded");
  await expect(evidence).toContainText("Local literal imports");
  await expect(evidence).toContainText("package imports, aliases, and computed imports remain unknown");
  await expect(evidence).not.toContainText("Rust AST semantic index hit its bound");

  const coverage = drawer.locator(".repo-intel-coverage");
  await expect(coverage).toContainText("Semantic coverage");
  const typescript = coverage.locator(".repo-intel-coverage-row").filter({ hasText: "typescript" });
  await expect(typescript).toContainText("2/3");
  await expect(typescript).not.toContainText("index capped");
  const rust = coverage.locator(".repo-intel-coverage-row").filter({ hasText: "rust" });
  await expect(rust).toContainText("1/1");
  await expect(rust).toContainText("index capped");
  await expect(drawer).toContainText("src/api.ts");

  const invocation = (await recordedInvocations(page))
    .find((item) => item.cmd === "repository_intelligence_snapshot");
  expect(invocation?.args).toEqual({ focusPath: "src/app.ts" });
});
