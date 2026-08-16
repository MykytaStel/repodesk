from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one match, found {count}: {old[:80]!r}")
    file.write_text(text.replace(old, new, 1))


receipt = "crates/repodesk-core/src/workflow/receipt.rs"
replace_once(
    receipt,
    "use crate::errors::{RepoDeskError, RepoDeskResult};\n",
    "use crate::change_evidence::ChangeEvidenceStatus;\nuse crate::errors::{RepoDeskError, RepoDeskResult};\n",
)
replace_once(
    receipt,
    "    #[serde(default)]\n    pub changed_files: Vec<String>,\n}\n",
    "    #[serde(default)]\n    pub changed_files: Vec<String>,\n    /// Whether `changed_files` is complete evidence or only an unknown/unavailable placeholder.\n    #[serde(default)]\n    pub change_evidence_status: ChangeEvidenceStatus,\n}\n",
)
replace_once(
    receipt,
    "            required\n                .iter()\n                .all(|step| step.status == SubAgentStatus::Ok)\n",
    "            required.iter().all(|step| {\n                step.status == SubAgentStatus::Ok && step.change_evidence_status.is_complete()\n            })\n",
)
replace_once(
    receipt,
    "            allow_write,\n            changed_files: Vec::new(),\n        }\n",
    "            allow_write,\n            changed_files: Vec::new(),\n            change_evidence_status: ChangeEvidenceStatus::Complete,\n        }\n",
)

evidence = "crates/repodesk-core/src/orchestrator/execution_evidence.rs"
replace_once(
    evidence,
    "use crate::errors::{RepoDeskError, RepoDeskResult};\n",
    "use crate::change_evidence::ChangeEvidenceStatus;\nuse crate::errors::{RepoDeskError, RepoDeskResult};\n",
)
replace_once(
    evidence,
    "    /// The agent already ran, but the execution receipt is missing/unusable.\n    RecoveryRequired,\n    /// Dry runs intentionally carry no execution receipt.\n",
    "    /// The agent already ran, but the execution receipt is missing/unusable.\n    RecoveryRequired,\n    /// The receipt exists, but its captured changeset provenance is not review-safe.\n    Incomplete,\n    /// Dry runs intentionally carry no execution receipt.\n",
)
replace_once(
    evidence,
    "        ExecutionEvidenceStatus::RecoveryRequired => {\n            let detail = state\n",
    "        ExecutionEvidenceStatus::Incomplete => Err(routing_error(\n            \"review blocked: execution evidence is incomplete; rerun execution to obtain trustworthy changeset provenance\",\n        )),\n        ExecutionEvidenceStatus::RecoveryRequired => {\n            let detail = state\n",
)
replace_once(
    evidence,
    "                allow_write: allow_write_of(&result.task_id),\n                changed_files: result.changed_files.clone(),\n            }\n",
    "                allow_write: allow_write_of(&result.task_id),\n                changed_files: result.changed_files.clone(),\n                // Until executor provenance is threaded through SubAgentResult,\n                // stay conservative rather than upgrading an empty list to proof.\n                change_evidence_status: ChangeEvidenceStatus::LegacyUnknown,\n            }\n",
)
