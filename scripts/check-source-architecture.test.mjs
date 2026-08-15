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
    /ai_source_detect|ai_source_import|memory_list|memory_add|Project AI Import|Project Memory & Guidelines/,
    "global Settings must not own repository-specific knowledge inputs",
  );
  assert.match(
    knowledgeSource,
    /ai_source_detect/,
    "Projects Knowledge must own project AI source detection",
  );
  assert.match(
    knowledgeSource,
    /ai_source_import/,
    "Projects Knowledge must own project AI source import",
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
