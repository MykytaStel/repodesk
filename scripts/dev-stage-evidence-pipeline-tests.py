from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one match, found {count}: {old[:120]!r}")
    file.write_text(text.replace(old, new, 1))


def append_once(path: str, marker: str, addition: str) -> None:
    file = Path(path)
    text = file.read_text()
    if addition.strip() in text:
        raise SystemExit(f"{path}: addition already present")
    if marker not in text:
        raise SystemExit(f"{path}: marker missing: {marker!r}")
    file.write_text(text.replace(marker, marker + addition, 1))


evidence = "crates/repodesk-core/src/orchestrator/execution_evidence.rs"
replace_once(
    evidence,
    '                changed_files: vec!["src/lib.rs".into()],\n                diff_path: None,\n                workspace: None::<RunWorktree>,\n                notes: vec![],\n',
    '                changed_files: vec!["src/lib.rs".into()],\n                change_evidence_status: ChangeEvidenceStatus::Complete,\n                execution_issues: vec![],\n                diff_path: None,\n                workspace: None::<RunWorktree>,\n                notes: vec![],\n',
)
replace_once(
    evidence,
    '        assert!(receipt.execution.required_steps[0].allow_write);\n        assert_eq!(\n            receipt.execution.changeset_digest,\n',
    '        assert!(receipt.execution.required_steps[0].allow_write);\n        assert_eq!(\n            receipt.execution.required_steps[0].change_evidence_status,\n            ChangeEvidenceStatus::Complete\n        );\n        assert_eq!(matching_receipt_status(&receipt), ExecutionEvidenceStatus::Ready);\n        assert_eq!(\n            receipt.execution.changeset_digest,\n',
)
append_once(
    evidence,
    '    #[test]\n    fn receipt_mismatch_is_not_ready_evidence() {\n',
    '''    #[test]\n    fn matching_receipt_with_unavailable_write_evidence_is_incomplete() {\n        let mut run = run();\n        run.results[0].change_evidence_status = ChangeEvidenceStatus::Unavailable;\n        let plan = OrchestrationPlan {\n            project: run.project.clone(),\n            task_id: run.task_id.clone(),\n            goal: run.goal.clone(),\n            steps: vec![task("impl", true)],\n        };\n        let receipt =\n            build_execution_receipt(&plan, &run, ExecutionMode::AgentRun, Some("base".into()));\n\n        assert_eq!(\n            receipt.execution.required_steps[0].change_evidence_status,\n            ChangeEvidenceStatus::Unavailable\n        );\n        assert_eq!(\n            matching_receipt_status(&receipt),\n            ExecutionEvidenceStatus::Incomplete\n        );\n    }\n\n    #[test]\n    fn legacy_unknown_non_write_step_does_not_require_changeset_proof() {\n        let mut run = run();\n        run.results[0].change_evidence_status = ChangeEvidenceStatus::LegacyUnknown;\n        let plan = OrchestrationPlan {\n            project: run.project.clone(),\n            task_id: run.task_id.clone(),\n            goal: run.goal.clone(),\n            steps: vec![task("impl", false)],\n        };\n        let receipt =\n            build_execution_receipt(&plan, &run, ExecutionMode::AgentRun, Some("base".into()));\n\n        assert_eq!(matching_receipt_status(&receipt), ExecutionEvidenceStatus::Ready);\n    }\n\n''',
)

auto_loop = "crates/repodesk-core/src/orchestrator/auto_loop.rs"
replace_once(
    auto_loop,
    '        let (_, terminal) = classify(&RunStatus::Completed, false, false, false);\n        assert_eq!(terminal, Some(LoopStatus::Succeeded));\n',
    '        let (_, terminal) = classify(\n            &RunStatus::Completed,\n            false,\n            false,\n            ExecutionEvidenceStatus::Ready,\n        );\n        assert_eq!(terminal, Some(LoopStatus::Succeeded));\n',
)
replace_once(
    auto_loop,
    '        let (_, terminal) = classify(&RunStatus::Completed, false, false, true);\n        assert_eq!(terminal, Some(LoopStatus::EvidenceRecoveryRequired));\n',
    '        let (_, terminal) = classify(\n            &RunStatus::Completed,\n            false,\n            false,\n            ExecutionEvidenceStatus::RecoveryRequired,\n        );\n        assert_eq!(terminal, Some(LoopStatus::EvidenceRecoveryRequired));\n',
)
append_once(
    auto_loop,
    '    #[test]\n    fn guardrail_block_stops_without_retry() {\n',
    '''    #[test]\n    fn incomplete_evidence_retries_execution_instead_of_claiming_success() {\n        let (note, terminal) = classify(\n            &RunStatus::Completed,\n            false,\n            false,\n            ExecutionEvidenceStatus::Incomplete,\n        );\n        assert_eq!(terminal, None);\n        assert!(note.contains("rerun"));\n        assert!(!note.contains("repair"));\n    }\n\n''',
)

contract_test = "crates/repodesk-core/tests/execution_evidence_truth.rs"
contract = Path(contract_test).read_text()
contract += '''\n#[test]\nfn historical_subagent_result_defaults_to_unknown_evidence() {\n    let result: repodesk_core::orchestrator::SubAgentResult = serde_json::from_value(serde_json::json!({\n        "task_id": "legacy",\n        "agent": "manual",\n        "provider": "manual",\n        "model": "external",\n        "status": "ok",\n        "output": "done",\n        "input_tokens": 0,\n        "output_tokens": 0,\n        "cost_units": 0.0,\n        "captured_proposals": 0,\n        "changed_files": [],\n        "notes": []\n    }))\n    .expect("historical run results must remain loadable");\n\n    assert_eq!(\n        result.change_evidence_status,\n        ChangeEvidenceStatus::LegacyUnknown\n    );\n    assert!(result.execution_issues.is_empty());\n}\n'''
Path(contract_test).write_text(contract)

arch = "scripts/check-source-architecture.test.mjs"
arch_text = Path(arch).read_text()
arch_text += '''\n\ntest("execution evidence has one canonical receipt owner and no unknown-to-none copy", () => {\n  const runner = readFileSync(\n    new URL("../crates/repodesk-core/src/orchestrator/runner.rs", import.meta.url),\n    "utf8",\n  );\n  const evidence = readFileSync(\n    new URL("../crates/repodesk-core/src/orchestrator/execution_evidence.rs", import.meta.url),\n    "utf8",\n  );\n\n  assert.doesNotMatch(\n    runner,\n    /fn write_execution_receipt\\b|save_receipt\\s*\\(/,\n    "raw runner must not own canonical execution-receipt persistence",\n  );\n  assert.match(\n    evidence,\n    /save_receipt\\s*\\(/,\n    "execution_evidence must remain the canonical receipt finalization owner",\n  );\n  assert.doesNotMatch(\n    runner,\n    /no writes detected/,\n    "an empty path list must never be described as proven no-write evidence without provenance",\n  );\n});\n'''
Path(arch).write_text(arch_text)
