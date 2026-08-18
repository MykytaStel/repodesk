import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const MIGRATED_SHELL_FILES = [
  "apps/desktop/src/app/App.tsx",
  "apps/desktop/src/app/ActivityRail.tsx",
  "apps/desktop/src/app/WorkspaceSidebar.tsx",
  "apps/desktop/src/app/WorkspaceInspector.tsx",
  "apps/desktop/src/app/WorkbenchInspectorSurface.tsx",
];

function source(path) {
  return readFileSync(path, "utf8");
}

test("migrated workbench shell does not reintroduce drawer terminology", () => {
  for (const path of MIGRATED_SHELL_FILES) {
    assert.doesNotMatch(
      source(path),
      /\bdrawers?\b/i,
      `${path} must use Navigator/Inspector structural-surface terminology instead of drawer`,
    );
  }
});

test("migrated workbench shell never coerces structured errors with String(error)", () => {
  for (const path of MIGRATED_SHELL_FILES) {
    assert.doesNotMatch(
      source(path),
      /\bString\s*\(\s*[^)]*error[^)]*\)/i,
      `${path} must normalize user-visible structured errors instead of String(error)`,
    );
  }
});

test("shell exposes one canonical Navigator and Inspector contract", () => {
  const rail = source("apps/desktop/src/app/ActivityRail.tsx");
  const navigator = source("apps/desktop/src/app/WorkspaceSidebar.tsx");
  const inspector = source("apps/desktop/src/app/WorkbenchInspectorSurface.tsx");

  assert.match(rail, /Navigator — ⌘\/Ctrl\+B/);
  assert.match(navigator, /aria-label="Workspace navigator"/);
  assert.match(inspector, /aria-label="Close inspector"/);
  assert.match(inspector, /event\.key !== "Escape"/);
  assert.match(inspector, /aria-modal="true"/);
  assert.match(inspector, /opener\?\.isConnected/);
});
