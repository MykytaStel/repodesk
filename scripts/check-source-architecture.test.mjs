import assert from "node:assert/strict";
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
