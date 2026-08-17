import assert from "node:assert/strict";
import { readFileSync, statSync } from "node:fs";
import test from "node:test";
import * as architecture from "./check-source-architecture.mjs";
import {
  HARD_SOURCE_LIMIT_BYTES,
  evaluateCodeSemanticContract,
  evaluateProjectsSemanticContract,
  evaluateRunsSemanticContract,
  evaluateVisualDebtChange,
  evaluateWorkSemanticContract,
  evaluateWorkVisualDebtCleanupContract,
} from "./check-source-architecture.mjs";

function failures(change) {
  return evaluateVisualDebtChange({
    path: "apps/desktop/src/features/example/Example.tsx",
    baseText: "",
    currentText: "",
    baseSize: 0,
    currentSize: 0,
    ...change,
  });
}

test("raw TSX hex debt may stay flat or shrink but not grow", () => {
  assert.deepEqual(failures({ baseText: "const a = '#fff';", currentText: "const a = '#fff';" }), []);
  assert.deepEqual(failures({ baseText: "const a = '#fff';", currentText: "const a = 'token';" }), []);
  assert.match(
    failures({ baseText: "", currentText: "const a = '#fff';" }).join("\n"),
    /raw hex/i,
  );
});

test("static inline style debt may not grow in TSX", () => {
  assert.deepEqual(failures({ baseText: "<div style={{ width: 1 }} />", currentText: "<div style={{ width: 1 }} />" }), []);
  assert.match(
    failures({ baseText: "", currentText: "<div style={{ padding: 8 }} />" }).join("\n"),
    /inline style/i,
  );
});

test("statusTone consumers may not grow", () => {
  assert.deepEqual(failures({ baseText: "statusTone(value)", currentText: "statusTone(value)" }), []);
  assert.match(
    failures({ baseText: "", currentText: "statusTone(value)" }).join("\n"),
    /statusTone/i,
  );
});

test("new versioned and polish visual generations are rejected", () => {
  assert.match(
    failures({ path: "apps/desktop/src/app/styles/work-v4.css", baseSize: null, currentSize: 10 }).join("\n"),
    /versioned visual/i,
  );
  assert.match(
    failures({ path: "apps/desktop/src/features/work/work-polish.css", baseSize: null, currentSize: 10 }).join("\n"),
    /polish stylesheet/i,
  );
});

test("legacy polish styles are fully retired from desktop source", () => {
  const evaluate = architecture.evaluateLegacyPolishDebtCleanupContract;
  assert.equal(
    typeof evaluate,
    "function",
    "architecture ratchet must expose a generic legacy polish cleanup contract",
  );
  if (typeof evaluate !== "function") return;

  assert.match(
    evaluate(["apps/desktop/src/features/code/code-editor-polish.css"]).join("\n"),
    /legacy polish stylesheet/i,
  );
  assert.deepEqual(evaluate(["apps/desktop/src/app/styles/work-hierarchy-v4.css"]), []);
  assert.deepEqual(evaluate(["apps/desktop/src/app/styles/work-hierarchy.css"]), []);
  assert.deepEqual(evaluate(), []);
});

test("existing feature CSS may stay flat or shrink but not grow", () => {
  const path = "apps/desktop/src/features/changes/changes-density.css";
  assert.deepEqual(failures({ path, baseSize: 100, currentSize: 100 }), []);
  assert.deepEqual(failures({ path, baseSize: 100, currentSize: 80 }), []);
  assert.match(failures({ path, baseSize: 100, currentSize: 101 }).join("\n"), /feature CSS grew/i);
});

test("canonical shared primitive CSS can be introduced outside feature-local CSS", () => {
  assert.deepEqual(
    failures({
      path: "apps/desktop/src/shared/ui/primitives/primitives.css",
      baseSize: null,
      currentSize: 500,
    }),
    [],
  );
});

test("ErrorState detail wrapper is block-safe for structured blocker evidence", () => {
  const source = readFileSync("apps/desktop/src/shared/ui/primitives/ErrorState.tsx", "utf8");
  assert.doesNotMatch(source, /<span>\{detail\}<\/span>/);
  assert.match(source, /className="semantic-state__detail"/);
});

test("Work migration requires one typed adapter and the shared primitive boundary", () => {
  assert.deepEqual(evaluateWorkSemanticContract(), []);
});

test("Runs migration requires one typed adapter and the shared primitive boundary", () => {
  assert.deepEqual(evaluateRunsSemanticContract(), []);
});

test("Projects migration requires one typed adapter and the shared primitive boundary", () => {
  assert.deepEqual(evaluateProjectsSemanticContract(), []);
});

test("Code migration requires one typed adapter and the shared primitive boundary", () => {
  assert.deepEqual(evaluateCodeSemanticContract(), []);
});

test("Code route is below the 28 KiB source architecture limit after migration", () => {
  assert.ok(
    statSync("apps/desktop/src/features/code/CodeTab.tsx").size <= HARD_SOURCE_LIMIT_BYTES,
    "CodeTab.tsx must be at or below the hard source limit after Code convergence",
  );
});

test("Work visual ownership is canonical after semantic convergence", () => {
  assert.deepEqual(evaluateWorkVisualDebtCleanupContract(), []);
});

test("the grandfathered Work progress width stays the only explicit dynamic inline-style exception", () => {
  const source = readFileSync("apps/desktop/src/features/work/WorkSurface.tsx", "utf8");
  const inlineStyles = source.match(/\bstyle\s*=\s*\{\s*\{/g) ?? [];
  assert.equal(inlineStyles.length, 1);
  assert.match(source, /style=\{\{ width: `\$\{phasePercent\}%` \}\}/);
});