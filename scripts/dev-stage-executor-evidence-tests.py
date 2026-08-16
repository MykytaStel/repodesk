from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one match, found {count}: {old[:80]!r}")
    file.write_text(text.replace(old, new, 1))


tests = "crates/repodesk-core/src/executors/tests.rs"
replace_once(
    tests,
    '    assert_eq!(result.status, "ok");\n    let paths: Vec<&str> = result\n',
    '    assert_eq!(result.status, "ok");\n    assert_eq!(\n        result.change_evidence_status,\n        crate::change_evidence::ChangeEvidenceStatus::Complete\n    );\n    let paths: Vec<&str> = result\n',
)
replace_once(
    tests,
    '    assert!(result.stdout.contains("agent-finished"));\n    assert!(result.changed_files.is_empty());\n}\n',
    '    assert!(result.stdout.contains("agent-finished"));\n    assert!(result.changed_files.is_empty());\n    assert_eq!(\n        result.change_evidence_status,\n        crate::change_evidence::ChangeEvidenceStatus::Unavailable\n    );\n}\n',
)

runtime = "crates/repodesk-core/src/executors/runtime.rs"
replace_once(
    runtime,
    '''    fn bounded_text_respects_utf8_boundaries() {
        let input = "💾".repeat(20).into_bytes();
        let (text, truncated) = bounded_text(input, true, 31);
        assert!(truncated);
        assert!(text.is_char_boundary(text.len()));
        assert!(text.len() <= 31);
    }
}''',
    '''    fn bounded_text_respects_utf8_boundaries() {
        let input = "💾".repeat(20).into_bytes();
        let (text, truncated) = bounded_text(input, true, 31);
        assert!(truncated);
        assert!(text.is_char_boundary(text.len()));
        assert!(text.len() <= 31);
    }

    #[test]
    fn execution_issues_are_secret_redacted_and_bounded() {
        let mut issues = Vec::new();
        for index in 0..(MAX_EXECUTION_ISSUES + 4) {
            push_execution_issue(
                &mut issues,
                format!("api_key=abcdefghijklmnopqrstuvwxyz{index:02}"),
            );
        }

        assert!(issues.len() <= MAX_EXECUTION_ISSUES);
        assert!(
            issues
                .iter()
                .all(|issue| issue.len() <= MAX_EXECUTION_ISSUE_BYTES)
        );
        assert!(issues.iter().all(|issue| !issue.contains("abcdefghijklmnopqrstuvwxyz")));
    }
}''',
)
