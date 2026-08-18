import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const workflow = await readFile(new URL("../.github/workflows/ci.yml", import.meta.url), "utf8");

test("supply-chain CI authenticates and verifies cargo-deny provisioning", () => {
  assert.doesNotMatch(
    workflow,
    /EmbarkStudios\/cargo-deny-action@/,
    "the Docker action downloads cargo-deny from GitHub Releases without authentication during image build",
  );
  assert.match(workflow, /CARGO_DENY_VERSION:\s*["']?\d+\.\d+\.\d+["']?/);
  assert.match(workflow, /CARGO_DENY_SHA256:\s*["']?[a-f0-9]{64}["']?/);
  assert.match(workflow, /GH_TOKEN:\s*\$\{\{\s*github\.token\s*\}\}/);
  assert.match(workflow, /Authorization:\s*Bearer \$\{GH_TOKEN\}/);
  assert.match(workflow, /sha256sum --check/);
  assert.match(workflow, /cargo deny check/);
});
