from pathlib import Path
import re


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one match, found {count}: {old[:120]!r}")
    file.write_text(text.replace(old, new, 1))


def replace_regex_once(path: str, pattern: str, replacement: str) -> None:
    file = Path(path)
    text = file.read_text()
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.S)
    if count != 1:
        raise SystemExit(f"{path}: expected one regex match, found {count}: {pattern[:120]!r}")
    file.write_text(updated)


types = "crates/repodesk-core/src/orchestrator/types.rs"
replace_once(
    types,
    "use crate::api_clients::ThinkingLevel;\n",
    "use crate::api_clients::ThinkingLevel;\nuse crate::change_evidence::ChangeEvidenceStatus;\n",
)
replace_once(
    types,
    "    #[serde(default)]\n    pub changed_files: Vec<String>,\n    /// Receipt file holding the full captured diff for a coding-agent step.\n",
    "    #[serde(default)]\n    pub changed_files: Vec<String>,\n    /// Whether `changed_files` is complete changeset evidence. Historical run\n    /// JSON without this field remains conservative (`legacy_unknown`).\n    #[serde(default)]\n    pub change_evidence_status: ChangeEvidenceStatus,\n    /// Bounded, secret-redacted execution diagnostics that must survive the\n    /// executor boundary without being flattened into misleading notes.\n    #[serde(default)]\n    pub execution_issues: Vec<String>,\n    /// Receipt file holding the full captured diff for a coding-agent step.\n",
)

runner = "crates/repodesk-core/src/orchestrator/runner.rs"
replace_once(
    runner,
    "use crate::api_clients::{LlmRequest, LlmResponse, ProviderSettings, provider_for};\n",
    "use crate::api_clients::{LlmRequest, LlmResponse, ProviderSettings, provider_for};\nuse crate::change_evidence::ChangeEvidenceStatus;\n",
)
replace_once(
    runner,
    "/// entering this execution boundary. All authoritative run/receipt evidence is\n/// still produced by the runner itself.\n",
    "/// entering this execution boundary. The raw runner persists run history; the\n/// public execution-evidence boundary owns canonical workflow-receipt finalization.\n",
)
replace_once(
    runner,
    '''                        let changed_files: Vec<String> = execution
                            .changed_files
                            .iter()
                            .map(|change| change.path.clone())
                            .collect();
                        if changed_files.is_empty() {
                            notes.push("changed files: none (no writes detected)".to_string());
                        } else {
                            notes.push(format!(
                                "changed files ({}): {}",
                                changed_files.len(),
                                changed_files.join(", ")
                            ));
                            if let Some(diff_path) = &execution.diff_path {
                                notes.push(format!(
                                    "diff: {diff_path}{}",
                                    if execution.diff_truncated {
                                        " (truncated)"
                                    } else {
                                        ""
                                    }
                                ));
                            }
                        }
''',
    '''                        let changed_files: Vec<String> = execution
                            .changed_files
                            .iter()
                            .map(|change| change.path.clone())
                            .collect();
                        match execution.change_evidence_status {
                            ChangeEvidenceStatus::Complete => {
                                if changed_files.is_empty() {
                                    notes.push(
                                        "changeset capture complete: no tracked file changes were produced"
                                            .to_string(),
                                    );
                                } else {
                                    notes.push(format!(
                                        "changed files ({}): {}",
                                        changed_files.len(),
                                        changed_files.join(", ")
                                    ));
                                    if let Some(diff_path) = &execution.diff_path {
                                        notes.push(format!(
                                            "diff: {diff_path}{}",
                                            if execution.diff_truncated {
                                                " (truncated)"
                                            } else {
                                                ""
                                            }
                                        ));
                                    }
                                }
                            }
                            ChangeEvidenceStatus::Unavailable => notes.push(
                                "change evidence unavailable: RepoDesk cannot prove which tracked paths changed"
                                    .to_string(),
                            ),
                            ChangeEvidenceStatus::LegacyUnknown => notes.push(
                                "change evidence unknown: rerun execution to capture a trustworthy changeset"
                                    .to_string(),
                            ),
                        }
''',
)
replace_once(
    runner,
    "                            captured_proposals: captured,\n                            changed_files,\n                            diff_path: execution.diff_path.clone(),\n",
    "                            captured_proposals: captured,\n                            changed_files,\n                            change_evidence_status: execution.change_evidence_status,\n                            execution_issues: execution.execution_issues.clone(),\n                            diff_path: execution.diff_path.clone(),\n",
)
replace_once(
    runner,
    '''    persist_run(&run).await?;
    // Write the evidence receipt: a fresh receipt for this run (which auto-
    // invalidates any stale review/verification from an earlier run). Dry runs
    // carry no evidence; a receipt error never fails a run that already completed.
    if !run.dry_run {
        let _ = write_execution_receipt(plan, &run);
    }
    // Record the outcome ledger (the N8 learning signal). Dry runs carry no
''',
    '''    persist_run(&run).await?;
    // Record the outcome ledger (the N8 learning signal). Dry runs carry no
''',
)
replace_once(
    runner,
    "        captured_proposals: 0,\n        changed_files: Vec::new(),\n        diff_path: None,\n",
    "        captured_proposals: 0,\n        changed_files: Vec::new(),\n        change_evidence_status: if step.allow_write {\n            ChangeEvidenceStatus::LegacyUnknown\n        } else {\n            ChangeEvidenceStatus::Complete\n        },\n        execution_issues: Vec::new(),\n        diff_path: None,\n",
)
replace_regex_once(
    runner,
    r'/// Persist a fresh \[`TaskRunReceipt`\].*?\nfn write_execution_receipt\(.*?\n}\n\nasync fn persist_run',
    'async fn persist_run',
)

manual_import = "crates/repodesk-core/src/orchestrator/manual_import.rs"
replace_once(
    manual_import,
    "        captured_proposals: 0,\n        changed_files: changed_files.clone(),\n        diff_path: None,\n",
    "        captured_proposals: 0,\n        changed_files: changed_files.clone(),\n        change_evidence_status: crate::change_evidence::ChangeEvidenceStatus::Complete,\n        execution_issues: Vec::new(),\n        diff_path: None,\n",
)

execution_evidence = "crates/repodesk-core/src/orchestrator/execution_evidence.rs"
replace_once(
    execution_evidence,
    '''        Ok(Some(receipt)) if execution_receipt_matches_run(&receipt, &run) => {
            return Ok(ExecutionEvidenceState {
                run_id: run_id.to_string(),
                status: ExecutionEvidenceStatus::Ready,
                recoverable: false,
                detail: None,
            });
        }
''',
    '''        Ok(Some(receipt)) if execution_receipt_matches_run(&receipt, &run) => {
            return Ok(matching_receipt_state(run_id, &receipt));
        }
''',
)
replace_once(
    execution_evidence,
    '''    if let Ok(Some(receipt)) = load_receipt_for_run(run_id)
        && execution_receipt_matches_run(&receipt, &run)
    {
        let _ = clear_recovery_record(run_id);
        return Ok(ExecutionEvidenceState {
            run_id: run_id.to_string(),
            status: ExecutionEvidenceStatus::Ready,
            recoverable: false,
            detail: None,
        });
    }
''',
    '''    if let Ok(Some(receipt)) = load_receipt_for_run(run_id)
        && execution_receipt_matches_run(&receipt, &run)
    {
        let _ = clear_recovery_record(run_id);
        return Ok(matching_receipt_state(run_id, &receipt));
    }
''',
)
replace_once(
    execution_evidence,
    '''    save_receipt(&record.receipt)?;
    let repaired = load_receipt_for_run(run_id)?
        .filter(|receipt| execution_receipt_matches_run(receipt, &run))
        .is_some();
    if !repaired {
        return Err(routing_error(
            "execution evidence repair did not produce a receipt bound to the persisted run",
        ));
    }
    let _ = clear_recovery_record(run_id);

    Ok(ExecutionEvidenceState {
        run_id: run_id.to_string(),
        status: ExecutionEvidenceStatus::Ready,
        recoverable: false,
        detail: None,
    })
''',
    '''    save_receipt(&record.receipt)?;
    let repaired = load_receipt_for_run(run_id)?
        .filter(|receipt| execution_receipt_matches_run(receipt, &run))
        .ok_or_else(|| {
            routing_error(
                "execution evidence repair did not produce a receipt bound to the persisted run",
            )
        })?;
    let _ = clear_recovery_record(run_id);

    Ok(matching_receipt_state(run_id, &repaired))
''',
)
replace_once(
    execution_evidence,
    '''    if let Some(receipt) = load_receipt_for_run(&run.run_id)?
        && execution_receipt_matches_run(&receipt, run)
    {
        let _ = clear_recovery_record(&run.run_id);
        return Ok(ExecutionEvidenceState {
            run_id: run.run_id.clone(),
            status: ExecutionEvidenceStatus::Ready,
            recoverable: false,
            detail: None,
        });
    }
''',
    '''    if let Some(receipt) = load_receipt_for_run(&run.run_id)?
        && execution_receipt_matches_run(&receipt, run)
    {
        let _ = clear_recovery_record(&run.run_id);
        return Ok(matching_receipt_state(&run.run_id, &receipt));
    }
''',
)
replace_once(
    execution_evidence,
    '''        Ok(()) => {
            let _ = clear_recovery_record(&run.run_id);
            Ok(ExecutionEvidenceState {
                run_id: run.run_id.clone(),
                status: ExecutionEvidenceStatus::Ready,
                recoverable: false,
                detail: None,
            })
        }
''',
    '''        Ok(()) => {
            let _ = clear_recovery_record(&run.run_id);
            Ok(matching_receipt_state(&run.run_id, &receipt))
        }
''',
)
replace_once(
    execution_evidence,
    '''                changed_files: result.changed_files.clone(),
                // Until executor provenance is threaded through SubAgentResult,
                // stay conservative rather than upgrading an empty list to proof.
                change_evidence_status: ChangeEvidenceStatus::LegacyUnknown,
''',
    '''                changed_files: result.changed_files.clone(),
                change_evidence_status: result.change_evidence_status,
''',
)
replace_once(
    execution_evidence,
    "        if step.status != result.status || step.changed_files != result.changed_files {\n            return false;\n        }\n",
    "        if step.status != result.status\n            || step.changed_files != result.changed_files\n            || step.change_evidence_status != result.change_evidence_status\n        {\n            return false;\n        }\n",
)
replace_once(
    execution_evidence,
    '''    let expected_digest = (!changed.is_empty()).then(|| changeset_digest(&changed));
    receipt.execution.changeset_digest == expected_digest
}

fn recovery_state(run_id: &str, detail: &str) -> RepoDeskResult<ExecutionEvidenceState> {
''',
    '''    let expected_digest = (!changed.is_empty()).then(|| changeset_digest(&changed));
    receipt.execution.changeset_digest == expected_digest
}

fn matching_receipt_status(receipt: &TaskRunReceipt) -> ExecutionEvidenceStatus {
    if receipt
        .execution
        .required_steps
        .iter()
        .any(|step| step.allow_write && !step.change_evidence_status.is_complete())
    {
        ExecutionEvidenceStatus::Incomplete
    } else {
        ExecutionEvidenceStatus::Ready
    }
}

fn matching_receipt_state(run_id: &str, receipt: &TaskRunReceipt) -> ExecutionEvidenceState {
    let status = matching_receipt_status(receipt);
    let detail = (status == ExecutionEvidenceStatus::Incomplete).then(|| {
        "execution receipt exists, but one or more write-capable steps lack complete changeset provenance; rerun execution before Review"
            .to_string()
    });
    ExecutionEvidenceState {
        run_id: run_id.to_string(),
        status,
        recoverable: false,
        detail,
    }
}

fn recovery_state(run_id: &str, detail: &str) -> RepoDeskResult<ExecutionEvidenceState> {
''',
)

auto_loop = "crates/repodesk-core/src/orchestrator/auto_loop.rs"
replace_once(
    auto_loop,
    '''        let evidence_recovery_required = !opts.dry_run
            && evidence_state_for_run(&run.run_id)?.status
                == ExecutionEvidenceStatus::RecoveryRequired;

        let (note, terminal) = classify(
            &run.status,
            guardrail_hit,
            opts.dry_run,
            evidence_recovery_required,
        );
''',
    '''        let evidence_status = if opts.dry_run {
            ExecutionEvidenceStatus::NotRequired
        } else {
            evidence_state_for_run(&run.run_id)?.status
        };

        let (note, terminal) = classify(
            &run.status,
            guardrail_hit,
            opts.dry_run,
            evidence_status,
        );
''',
)
replace_once(
    auto_loop,
    '''fn classify(
    run_status: &RunStatus,
    guardrail_hit: bool,
    dry_run: bool,
    evidence_recovery_required: bool,
) -> (&'static str, Option<LoopStatus>) {
''',
    '''fn classify(
    run_status: &RunStatus,
    guardrail_hit: bool,
    dry_run: bool,
    evidence_status: ExecutionEvidenceStatus,
) -> (&'static str, Option<LoopStatus>) {
''',
)
replace_once(
    auto_loop,
    '''    if evidence_recovery_required {
        return (
            "execution completed but evidence persistence needs repair — do not rerun the agent",
            Some(LoopStatus::EvidenceRecoveryRequired),
        );
    }
    if guardrail_hit {
''',
    '''    if evidence_status == ExecutionEvidenceStatus::RecoveryRequired {
        return (
            "execution completed but evidence persistence needs repair — do not rerun the agent",
            Some(LoopStatus::EvidenceRecoveryRequired),
        );
    }
    if guardrail_hit {
''',
)
replace_once(
    auto_loop,
    '''    if guardrail_hit {
        return (
            "a safety/budget guardrail blocked a step — needs human intervention",
            Some(LoopStatus::GuardrailBlocked),
        );
    }
    match run_status {
''',
    '''    if guardrail_hit {
        return (
            "a safety/budget guardrail blocked a step — needs human intervention",
            Some(LoopStatus::GuardrailBlocked),
        );
    }
    if evidence_status == ExecutionEvidenceStatus::Incomplete {
        return (
            "execution evidence is incomplete — rerun execution to capture trustworthy changeset provenance",
            None,
        );
    }
    match run_status {
''',
)
replace_once(
    auto_loop,
    "        let (_, terminal) = classify(&RunStatus::DryRun, false, true, false);\n",
    "        let (_, terminal) = classify(\n            &RunStatus::DryRun,\n            false,\n            true,\n            ExecutionEvidenceStatus::NotRequired,\n        );\n",
)
replace_once(
    auto_loop,
    "        let (_, terminal) = classify(&RunStatus::Partial, true, false, false);\n",
    "        let (_, terminal) = classify(\n            &RunStatus::Partial,\n            true,\n            false,\n            ExecutionEvidenceStatus::Incomplete,\n        );\n",
)
replace_once(
    auto_loop,
    "        let (_, terminal) = classify(&RunStatus::Partial, false, false, false);\n",
    "        let (_, terminal) = classify(\n            &RunStatus::Partial,\n            false,\n            false,\n            ExecutionEvidenceStatus::Ready,\n        );\n",
)
