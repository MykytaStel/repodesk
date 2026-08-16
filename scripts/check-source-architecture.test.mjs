import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import test from "node:test";
import { HARD_SOURCE_LIMIT_BYTES, evaluateSourceChange } from "./check-source-architecture.mjs";

const limit = HARD_SOURCE_LIMIT_BYTES;

test("new source files must stay below the hard limit", () => {
  assert.equal(evaluateSourceChange({ path: "new.rs", baseSize: null, currentSize: limit }), null);
  assert.match(
    evaluateSourceChange({ path: "new.rs", baseSize: null, currentSize: limit + 1 }),
    /new source file/,
  );
});

test("files below the limit may grow without crossing it", () => {
  assert.equal(
    evaluateSourceChange({ path: "small.ts", baseSize: 1_000, currentSize: limit }),
    null,
  );
  assert.match(
    evaluateSourceChange({ path: "small.ts", baseSize: limit, currentSize: limit + 1 }),
    /grew across/,
  );
});

test("grandfathered god-files cannot grow", () => {
  assert.equal(
    evaluateSourceChange({ path: "legacy.rs", baseSize: limit + 2_000, currentSize: limit + 2_000 }),
    null,
  );
  assert.equal(
    evaluateSourceChange({ path: "legacy.rs", baseSize: limit + 2_000, currentSize: limit + 1_000 }),
    null,
  );
  assert.match(
    evaluateSourceChange({ path: "legacy.rs", baseSize: limit + 2_000, currentSize: limit + 2_001 }),
    /god-file grew/,
  );
});

test("deleted source files never block the ratchet", () => {
  assert.equal(
    evaluateSourceChange({ path: "deleted.tsx", baseSize: limit + 10_000, currentSize: null }),
    null,
  );
});

test("Projects owns repository project setup instead of global Settings", () => {
  const settingsTab = readFileSync(
    new URL("../apps/desktop/src/features/settings/SettingsTab.tsx", import.meta.url),
    "utf8",
  );
  const settingsHook = readFileSync(
    new URL("../apps/desktop/src/features/settings/useSettings.ts", import.meta.url),
    "utf8",
  );
  const projectsTab = readFileSync(
    new URL("../apps/desktop/src/features/projects/ProjectsTab.tsx", import.meta.url),
    "utf8",
  );
  const projectsDir = new URL("../apps/desktop/src/features/projects/", import.meta.url);
  const projectDomainSource = readdirSync(projectsDir)
    .filter((name) => /\.(?:ts|tsx)$/.test(name))
    .map((name) => readFileSync(new URL(name, projectsDir), "utf8"))
    .join("\n");

  assert.doesNotMatch(
    settingsTab,
    /Connect a project|addProjectFromSetup/,
    "Settings must not render or bind repository project setup",
  );
  assert.doesNotMatch(
    settingsHook,
    /project_add|project_use/,
    "settings domain must not own project registration or activation commands",
  );
  assert.match(
    projectDomainSource,
    /project_add/,
    "Projects domain must own project registration",
  );
  assert.match(
    projectDomainSource,
    /project_use/,
    "Projects domain must own project activation",
  );
  assert.doesNotMatch(
    projectsTab,
    /setActiveTab\("settings"/,
    "Projects setup/configuration must not redirect into global Settings",
  );
});

test("Projects Knowledge owns project-scoped AI import and legacy guidelines", () => {
  const settingsDir = new URL("../apps/desktop/src/features/settings/", import.meta.url);
  const settingsSource = readdirSync(settingsDir)
    .filter((name) => /\.(?:ts|tsx)$/.test(name))
    .map((name) => readFileSync(new URL(name, settingsDir), "utf8"))
    .join("\n");
  const knowledgeDir = new URL("../apps/desktop/src/features/knowledge/", import.meta.url);
  const knowledgeSource = readdirSync(knowledgeDir)
    .filter((name) => /\.(?:ts|tsx)$/.test(name))
    .map((name) => readFileSync(new URL(name, knowledgeDir), "utf8"))
    .join("\n");

  assert.doesNotMatch(
    settingsSource,
    /ProjectAiImportPanel|memory_list|memory_add|Project Memory & Guidelines/,
    "global Settings must not own repository-specific knowledge inputs",
  );
  assert.match(
    knowledgeSource,
    /ProjectAiImportPanel|projectAiScan/,
    "Projects Knowledge must own project AI import",
  );
  assert.match(
    knowledgeSource,
    /memory_list/,
    "Projects Knowledge must own legacy project guideline retrieval",
  );
  assert.match(
    knowledgeSource,
    /memory_add/,
    "Projects Knowledge must own legacy project guideline writes",
  );
});

test("Credentials have one user-triggered mutation owner", () => {
  const settingsTab = readFileSync(
    new URL("../apps/desktop/src/features/settings/SettingsTab.tsx", import.meta.url),
    "utf8",
  );
  const settingsHook = readFileSync(
    new URL("../apps/desktop/src/features/settings/useSettings.ts", import.meta.url),
    "utf8",
  );
  const routingApi = readFileSync(
    new URL("../apps/desktop/src/shared/api/routing.ts", import.meta.url),
    "utf8",
  );
  const credentialsApi = readFileSync(
    new URL("../apps/desktop/src/shared/api/credentials.ts", import.meta.url),
    "utf8",
  );
  const tauriLib = readFileSync(
    new URL("../apps/desktop/src-tauri/src/lib.rs", import.meta.url),
    "utf8",
  );

  assert.doesNotMatch(
    settingsTab,
    /Save API keys/,
    "Settings must expose only the dedicated credential editor, not a second generic API-key save action",
  );
  assert.doesNotMatch(
    settingsHook,
    /saveApiKeys|keyDraft|anthropic_api_key|openai_api_key|gemini_api_key/,
    "generic Settings state must not own provider secret drafts or map secrets into provider preferences",
  );
  assert.doesNotMatch(
    routingApi,
    /save_provider_settings|\b(?:anthropic|openai|gemini)_api_key\??\s*:/,
    "routing/provider preferences must be a non-secret IPC contract",
  );
  assert.match(credentialsApi, /credential_set/);
  assert.match(credentialsApi, /credential_delete/);
  assert.doesNotMatch(
    tauriLib,
    /commands::save_provider_settings/,
    "the current Tauri invoke surface must not expose the legacy secret-bearing provider settings writer",
  );
  assert.match(
    tauriLib,
    /commands::save_provider_preferences/,
    "the Tauri invoke surface must expose a non-secret provider preference writer",
  );
});


test("execution evidence has one canonical receipt owner and no unknown-to-none copy", () => {
  const runner = readFileSync(
    new URL("../crates/repodesk-core/src/orchestrator/runner.rs", import.meta.url),
    "utf8",
  );
  const evidence = readFileSync(
    new URL("../crates/repodesk-core/src/orchestrator/execution_evidence.rs", import.meta.url),
    "utf8",
  );

  assert.doesNotMatch(
    runner,
    /fn write_execution_receipt\b|save_receipt\s*\(/,
    "raw runner must not own canonical execution-receipt persistence",
  );
  assert.match(
    evidence,
    /save_receipt\s*\(/,
    "execution_evidence must remain the canonical receipt finalization owner",
  );
  assert.doesNotMatch(
    runner,
    /no writes detected/,
    "an empty path list must never be described as proven no-write evidence without provenance",
  );
});


test("reserved run ids still pass through the canonical execution-evidence boundary", () => {
  const orchestrator = readFileSync(
    new URL("../crates/repodesk-core/src/orchestrator/mod.rs", import.meta.url),
    "utf8",
  );
  const evidence = readFileSync(
    new URL("../crates/repodesk-core/src/orchestrator/execution_evidence.rs", import.meta.url),
    "utf8",
  );

  assert.match(
    evidence,
    /pub async fn run_plan_with_id\b/,
    "reserved-id execution must have an evidence-aware public wrapper",
  );
  assert.match(
    orchestrator,
    /pub use execution_evidence::\{[^}]*run_plan_with_id/s,
    "the public reserved-id API must be exported from execution_evidence",
  );
  assert.doesNotMatch(
    orchestrator,
    /pub use runner::\{[^}]*run_plan_with_id/s,
    "raw runner reserved-id execution must not bypass receipt finalization",
  );
});
