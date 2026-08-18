import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const workflow = await readFile(new URL("../.github/workflows/ci.yml", import.meta.url), "utf8");

test("supply-chain CI provisions pinned cargo-deny without GitHub Release downloads", () => {
  assert.doesNotMatch(
    workflow,
    /EmbarkStudios\/cargo-deny-action@/,
    "the Docker action downloads cargo-deny from GitHub Releases during image build",
  );
  assert.match(workflow, /CARGO_DENY_VERSION:\s*["']?\d+\.\d+\.\d+["']?/);
  assert.match(workflow, /cargo install --locked cargo-deny/);
  assert.match(workflow, /--version "\$\{CARGO_DENY_VERSION\}"/);
  assert.match(workflow, /cargo deny --version/);
  assert.match(workflow, /cargo deny check/);
});
