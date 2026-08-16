from pathlib import Path

path = Path("apps/desktop/src/features/orchestrate/OrchestrateTab.tsx")
text = path.read_text()

old_tone = '''const LOOP_TONE: Record<LoopStatus, "ok" | "warn" | "danger" | "accent"> = {
  succeeded: "ok",
  needs_approval: "warn",
  guardrail_blocked: "danger",
  exhausted: "warn",
  dry_run: "accent",
};
'''
new_tone = '''const LOOP_TONE: Record<LoopStatus, "ok" | "warn" | "danger" | "accent"> = {
  succeeded: "ok",
  needs_approval: "warn",
  guardrail_blocked: "danger",
  evidence_recovery_required: "warn",
  exhausted: "warn",
  dry_run: "accent",
};
'''

old_hint = '''const LOOP_HINT: Record<LoopStatus, string> = {
  succeeded: "An attempt completed every step.",
  needs_approval: "The plan includes gated steps — enable the matching approvals to run it.",
  guardrail_blocked: "A safety/budget guardrail stopped the loop — resolve it, then re-run.",
  exhausted: "Out of attempts or budget before succeeding — raise the limits and re-run.",
  dry_run: "Preview only — nothing was executed.",
};
'''
new_hint = '''const LOOP_HINT: Record<LoopStatus, string> = {
  succeeded: "An attempt completed every step.",
  needs_approval: "The plan includes gated steps — enable the matching approvals to run it.",
  guardrail_blocked: "A safety/budget guardrail stopped the loop — resolve it, then re-run.",
  evidence_recovery_required:
    "Execution finished; repair the persisted evidence receipt before Review. Do not rerun the agent.",
  exhausted: "Out of attempts or budget before succeeding — raise the limits and re-run.",
  dry_run: "Preview only — nothing was executed.",
};
'''

for old, new, label in [
    (old_tone, new_tone, "loop tone map"),
    (old_hint, new_hint, "loop hint map"),
]:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"expected exactly one {label}, found {count}")
    text = text.replace(old, new, 1)

path.write_text(text)
