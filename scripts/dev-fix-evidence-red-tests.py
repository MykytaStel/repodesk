from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one match, found {count}: {old[:120]!r}")
    file.write_text(text.replace(old, new, 1))


auto_loop = "crates/repodesk-core/src/orchestrator/auto_loop.rs"
replace_once(
    auto_loop,
    '''    #[test]
    fn guardrail_block_stops_without_retry() {
        #[test]
        fn incomplete_evidence_retries_execution_instead_of_claiming_success() {
            let (note, terminal) = classify(
                &RunStatus::Completed,
                false,
                false,
                ExecutionEvidenceStatus::Incomplete,
            );
            assert_eq!(terminal, None);
            assert!(note.contains("rerun"));
            assert!(!note.contains("repair"));
        }

        let (_, terminal) = classify(&RunStatus::Partial, true, false, false);
        assert_eq!(terminal, Some(LoopStatus::GuardrailBlocked));
    }
''',
    '''    #[test]
    fn incomplete_evidence_retries_execution_instead_of_claiming_success() {
        let (note, terminal) = classify(
            &RunStatus::Completed,
            false,
            false,
            ExecutionEvidenceStatus::Incomplete,
        );
        assert_eq!(terminal, None);
        assert!(note.contains("rerun"));
        assert!(!note.contains("repair"));
    }

    #[test]
    fn guardrail_block_stops_without_retry() {
        let (_, terminal) = classify(&RunStatus::Partial, true, false, false);
        assert_eq!(terminal, Some(LoopStatus::GuardrailBlocked));
    }
''',
)

evidence = "crates/repodesk-core/src/orchestrator/execution_evidence.rs"
replace_once(
    evidence,
    '''    #[test]
    fn receipt_mismatch_is_not_ready_evidence() {
        #[test]
        fn matching_receipt_with_unavailable_write_evidence_is_incomplete() {
            let mut run = run();
            run.results[0].change_evidence_status = ChangeEvidenceStatus::Unavailable;
            let plan = OrchestrationPlan {
                project: run.project.clone(),
                task_id: run.task_id.clone(),
                goal: run.goal.clone(),
                steps: vec![task("impl", true)],
            };
            let receipt =
                build_execution_receipt(&plan, &run, ExecutionMode::AgentRun, Some("base".into()));

            assert_eq!(
                receipt.execution.required_steps[0].change_evidence_status,
                ChangeEvidenceStatus::Unavailable
            );
            assert_eq!(
                matching_receipt_status(&receipt),
                ExecutionEvidenceStatus::Incomplete
            );
        }

        #[test]
        fn legacy_unknown_non_write_step_does_not_require_changeset_proof() {
            let mut run = run();
            run.results[0].change_evidence_status = ChangeEvidenceStatus::LegacyUnknown;
            let plan = OrchestrationPlan {
                project: run.project.clone(),
                task_id: run.task_id.clone(),
                goal: run.goal.clone(),
                steps: vec![task("impl", false)],
            };
            let receipt =
                build_execution_receipt(&plan, &run, ExecutionMode::AgentRun, Some("base".into()));

            assert_eq!(
                matching_receipt_status(&receipt),
                ExecutionEvidenceStatus::Ready
            );
        }

        let run = run();
''',
    '''    #[test]
    fn matching_receipt_with_unavailable_write_evidence_is_incomplete() {
        let mut run = run();
        run.results[0].change_evidence_status = ChangeEvidenceStatus::Unavailable;
        let plan = OrchestrationPlan {
            project: run.project.clone(),
            task_id: run.task_id.clone(),
            goal: run.goal.clone(),
            steps: vec![task("impl", true)],
        };
        let receipt =
            build_execution_receipt(&plan, &run, ExecutionMode::AgentRun, Some("base".into()));

        assert_eq!(
            receipt.execution.required_steps[0].change_evidence_status,
            ChangeEvidenceStatus::Unavailable
        );
        assert_eq!(
            matching_receipt_status(&receipt),
            ExecutionEvidenceStatus::Incomplete
        );
    }

    #[test]
    fn legacy_unknown_non_write_step_does_not_require_changeset_proof() {
        let mut run = run();
        run.results[0].change_evidence_status = ChangeEvidenceStatus::LegacyUnknown;
        let plan = OrchestrationPlan {
            project: run.project.clone(),
            task_id: run.task_id.clone(),
            goal: run.goal.clone(),
            steps: vec![task("impl", false)],
        };
        let receipt =
            build_execution_receipt(&plan, &run, ExecutionMode::AgentRun, Some("base".into()));

        assert_eq!(
            matching_receipt_status(&receipt),
            ExecutionEvidenceStatus::Ready
        );
    }

    #[test]
    fn receipt_mismatch_is_not_ready_evidence() {
        let run = run();
''',
)
