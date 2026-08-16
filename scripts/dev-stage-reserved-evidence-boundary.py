from pathlib import Path

path = Path("scripts/check-source-architecture.test.mjs")
text = path.read_text()
addition = r'''

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
'''
if "reserved run ids still pass through" in text:
    raise SystemExit("regression already staged")
path.write_text(text + addition)
