from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one match, found {count}: {old[:120]!r}")
    file.write_text(text.replace(old, new, 1))


for path, old, new in [
    (
        "crates/repodesk-core/src/engineering/domain.rs",
        '''            changed_files: changed_files
                .iter()
                .map(|path| (*path).to_string())
                .collect(),
            diff_path: diff_path.map(str::to_string),
''',
        '''            changed_files: changed_files
                .iter()
                .map(|path| (*path).to_string())
                .collect(),
            change_evidence_status: crate::change_evidence::ChangeEvidenceStatus::Complete,
            execution_issues: Vec::new(),
            diff_path: diff_path.map(str::to_string),
''',
    ),
    (
        "crates/repodesk-core/src/orchestrator/context.rs",
        '''            captured_proposals: 0,
            changed_files: Vec::new(),
            diff_path: None,
''',
        '''            captured_proposals: 0,
            changed_files: Vec::new(),
            change_evidence_status: crate::change_evidence::ChangeEvidenceStatus::Complete,
            execution_issues: Vec::new(),
            diff_path: None,
''',
    ),
    (
        "crates/repodesk-core/src/orchestrator/review.rs",
        '''            captured_proposals: 0,
            changed_files: paths.iter().map(|path| path.to_string()).collect(),
            diff_path: None,
''',
        '''            captured_proposals: 0,
            changed_files: paths.iter().map(|path| path.to_string()).collect(),
            change_evidence_status: crate::change_evidence::ChangeEvidenceStatus::Complete,
            execution_issues: Vec::new(),
            diff_path: None,
''',
    ),
    (
        "crates/repodesk-core/src/orchestrator/review_transaction.rs",
        '''            changed_files: vec![
                "src/old.rs -> src/new.rs".into(),
                "../escape -> src/safe.rs".into(),
            ],
            diff_path: None,
''',
        '''            changed_files: vec![
                "src/old.rs -> src/new.rs".into(),
                "../escape -> src/safe.rs".into(),
            ],
            change_evidence_status: crate::change_evidence::ChangeEvidenceStatus::Complete,
            execution_issues: Vec::new(),
            diff_path: None,
''',
    ),
]:
    replace_once(path, old, new)
