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
