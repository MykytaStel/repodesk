from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one match, found {count}: {old[:100]!r}")
    file.write_text(text.replace(old, new, 1))


executors = "crates/repodesk-core/src/executors.rs"
replace_once(
    executors,
    "use crate::errors::{RepoDeskError, RepoDeskResult};\n",
    "use crate::change_evidence::ChangeEvidenceStatus;\nuse crate::errors::{RepoDeskError, RepoDeskResult};\n",
)
replace_once(
    executors,
    "    #[serde(default)]\n    pub execution_issues: Vec<String>,\n    /// Distinct secret kinds redacted out of the in-record output, if any.\n",
    "    #[serde(default)]\n    pub execution_issues: Vec<String>,\n    /// Whether the repository changeset was captured completely. Missing fields\n    /// in historical execution JSON remain conservative rather than becoming proof.\n    #[serde(default)]\n    pub change_evidence_status: ChangeEvidenceStatus,\n    /// Distinct secret kinds redacted out of the in-record output, if any.\n",
)

runtime = "crates/repodesk-core/src/executors/runtime.rs"
replace_once(
    runtime,
    "use crate::errors::{RepoDeskError, RepoDeskResult};\n",
    "use crate::change_evidence::ChangeEvidenceStatus;\nuse crate::errors::{RepoDeskError, RepoDeskResult};\n",
)
replace_once(
    runtime,
    'const OUTPUT_TRUNCATION_MARKER: &str = "\\n[output truncated]";\n',
    'const OUTPUT_TRUNCATION_MARKER: &str = "\\n[output truncated]";\nconst MAX_EXECUTION_ISSUES: usize = 16;\nconst MAX_EXECUTION_ISSUE_BYTES: usize = 512;\n',
)
replace_once(
    runtime,
    "    let changeset = match capture_changeset(cwd, output_dir, &safe_id, stamp, pre_status.as_ref()) {\n        Ok(changeset) => changeset,\n        Err(error) => {\n            push_execution_issue(\n                &mut execution_issues,\n                format!(\"changeset capture failed: {error}\"),\n            );\n            force_failed = true;\n            Changeset::empty()\n        }\n    };\n",
    "    let (changeset, change_evidence_status) =\n        match capture_changeset(cwd, output_dir, &safe_id, stamp, pre_status.as_ref()) {\n            Ok(changeset) => (changeset, ChangeEvidenceStatus::Complete),\n            Err(error) => {\n                push_execution_issue(\n                    &mut execution_issues,\n                    format!(\"changeset capture failed: {error}\"),\n                );\n                force_failed = true;\n                (Changeset::empty(), ChangeEvidenceStatus::Unavailable)\n            }\n        };\n",
)
replace_once(
    runtime,
    "        output_capture_issues,\n        execution_issues,\n        secrets_redacted,\n",
    "        output_capture_issues,\n        execution_issues,\n        change_evidence_status,\n        secrets_redacted,\n",
)
replace_once(
    runtime,
    "fn push_execution_issue(issues: &mut Vec<String>, issue: String) {\n    issues.push(issue);\n    issues.sort();\n    issues.dedup();\n}\n",
    "fn push_execution_issue(issues: &mut Vec<String>, issue: String) {\n    let (redacted, _) = crate::security::redact_secrets(&issue);\n    let bounded = truncate_char_boundary(&redacted, MAX_EXECUTION_ISSUE_BYTES).to_string();\n    if issues.iter().any(|existing| existing == &bounded) {\n        return;\n    }\n    if issues.len() >= MAX_EXECUTION_ISSUES {\n        return;\n    }\n    issues.push(bounded);\n    issues.sort();\n}\n",
)
replace_once(
    runtime,
    '        let mut issues = Vec::new();\n        for index in 0..(MAX_EXECUTION_ISSUES + 4) {\n            push_execution_issue(\n                &mut issues,\n                format!("api_key=abcdefghijklmnopqrstuvwxyz{index:02}"),\n            );\n        }\n',
    '        let mut issues = Vec::new();\n        let secret = ["sk-", "abcdefghijkl", "mnopqrstuvwx"].concat();\n        for index in 0..(MAX_EXECUTION_ISSUES + 4) {\n            push_execution_issue(&mut issues, format!("diagnostic {index}: {secret}"));\n        }\n',
)
replace_once(
    runtime,
    '                .all(|issue| !issue.contains("abcdefghijklmnopqrstuvwxyz"))\n',
    '                .all(|issue| !issue.contains("abcdefghijkl"))\n',
)
