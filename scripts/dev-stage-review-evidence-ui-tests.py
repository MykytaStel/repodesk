from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one match, found {count}: {old[:120]!r}")
    file.write_text(text.replace(old, new, 1))


fixtures = "apps/desktop/e2e/fixtures.ts"
replace_once(
    fixtures,
    '''        captured_proposals: 1,
        changed_files: ["src/app.ts"],
        notes: [],
''',
    '''        captured_proposals: 1,
        changed_files: ["src/app.ts"],
        change_evidence_status: "complete",
        execution_issues: [],
        notes: [],
''',
)
replace_once(
    fixtures,
    '''  orchestrate_run_diffs: [
''',
    '''  orchestrate_evidence_state: {
    run_id: "run-20260101-000000-1-0",
    status: "ready",
    recoverable: false,
    detail: null,
  },
  orchestrate_run_diffs: [
''',
)
file = Path(fixtures)
text = file.read_text()
addition = '''

export const incompleteReviewFixtures: CommandFixtures = {
  ...reviewFixtures,
  orchestrate_evidence_state: {
    run_id: "run-20260101-000000-1-0",
    status: "incomplete",
    recoverable: false,
    detail: "one or more write-capable steps lack complete changeset provenance",
  },
};

export const recoveryReviewFixtures: CommandFixtures = {
  ...reviewFixtures,
  orchestrate_evidence_state: {
    run_id: "run-20260101-000000-1-0",
    status: "recovery_required",
    recoverable: true,
    detail: "execution receipt persistence failed",
  },
};

export const zeroChangeReadyReviewFixtures: CommandFixtures = {
  ...reviewFixtures,
  orchestrate_evidence_state: {
    run_id: "run-20260101-000000-1-0",
    status: "ready",
    recoverable: false,
    detail: null,
  },
  orchestrate_run_diffs: [],
};
'''
if "incompleteReviewFixtures" in text:
    raise SystemExit("review evidence fixtures already staged")
file.write_text(text + addition)

spec = "apps/desktop/e2e/work-golden-path.spec.ts"
replace_once(
    spec,
    'import { firstRunFixtures, reviewFixtures } from "./fixtures";\n',
    '''import {
  firstRunFixtures,
  incompleteReviewFixtures,
  recoveryReviewFixtures,
  reviewFixtures,
  zeroChangeReadyReviewFixtures,
} from "./fixtures";
''',
)
replace_once(
    spec,
    '''    const commands = await recordedCommands(page);
    expect(commands).toContain("orchestrate_run_diffs");
    expect(commands).toContain("memory_proposals_list");
  });

  test("Review has evidence-bound Accept/Reject and no manual bypass", async ({ page }) => {
''',
    '''    const commands = await recordedCommands(page);
    expect(commands).toContain("orchestrate_evidence_state");
    expect(commands).toContain("orchestrate_run_diffs");
    expect(commands).toContain("memory_proposals_list");
  });

  test("Review blocks incomplete changeset evidence and does not fetch expensive diffs", async ({ page }) => {
    await installMockIpc(page, incompleteReviewFixtures);
    await page.goto("/");

    const alert = page.getByRole("alert");
    await expect(alert).toContainText("cannot prove which tracked paths changed");
    await expect(alert).toContainText("Rerun execution");
    await expect(page.getByText("No tracked file changes captured for this run.")).toHaveCount(0);

    const commands = await recordedCommands(page);
    expect(commands).toContain("orchestrate_evidence_state");
    expect(commands).not.toContain("orchestrate_run_diffs");
  });

  test("Review distinguishes receipt recovery from rerunning the agent", async ({ page }) => {
    await installMockIpc(page, recoveryReviewFixtures);
    await page.goto("/");

    const alert = page.getByRole("alert");
    await expect(alert).toContainText("persisted receipt needs repair");
    await expect(alert).toContainText("do not rerun the agent");

    const commands = await recordedCommands(page);
    expect(commands).toContain("orchestrate_evidence_state");
    expect(commands).not.toContain("orchestrate_run_diffs");
  });

  test("Review states proven zero-change evidence explicitly", async ({ page }) => {
    await installMockIpc(page, zeroChangeReadyReviewFixtures);
    await page.goto("/");

    await expect(page.getByText("Changeset capture is complete; no tracked file changes were produced.")).toBeVisible();
    await expect(page.getByText("No tracked file changes captured for this run.")).toHaveCount(0);
  });

  test("Review has evidence-bound Accept/Reject and no manual bypass", async ({ page }) => {
''',
)
