//! RepoDesk 2 engineering domain and event substrate.
//!
//! Keep this layer independent from frontend concerns. Legacy task/orchestrator
//! models adapt into these types until later migration slices move call sites.

pub mod acceptance_evidence;
pub mod algorithmic_profile;
pub mod change_governance;
pub mod commit_policy;
pub mod context_compactness;
pub mod context_inspector;
pub mod context_manifest;
pub mod domain;
pub mod events;
pub mod instrumentation;
pub mod intelligence;
pub mod knowledge;
pub mod run_evidence;
pub mod work_item_contract;

pub use acceptance_evidence::{
    ACCEPTANCE_EVIDENCE_FILE, ACCEPTANCE_EVIDENCE_VERSION, AcceptanceCriterionEvidence,
    AcceptanceCriterionStatus, AcceptanceEvidenceBinding, AcceptanceEvidenceReport,
    AcceptanceEvidenceStore, acceptance_evidence_path, active_verification_is_fresh, criterion_id,
    derive_acceptance_evidence, link_active_acceptance_evidence, load_active_acceptance_evidence,
    read_acceptance_evidence,
};
pub use algorithmic_profile::{
    AlgorithmicEvidence, AlgorithmicEvidenceKind, AlgorithmicProfile, AlgorithmicProfileError,
    AlgorithmicProfileReport, AlgorithmicSignals, AlgorithmicSymbolKind, AnalysisConfidence,
    ComplexityClass, ComplexityHint, MAX_ALGORITHM_SOURCE_BYTES, analyze_rust_file,
    analyze_rust_source,
};
pub use change_governance::{
    ChangeFileGovernance, ChangeFileScopeState, ChangeGovernanceSnapshot, ChangeOrigin,
    ChangeReviewState, ChangeVerificationEvidence, ChangeVerificationState, CommitGate,
    CommitGateState, ScopeOverrideEvidence, derive_change_governance,
    load_active_change_governance, load_change_governance, record_active_scope_override,
};
pub use commit_policy::{
    CommitScopePolicyDecision, derive_commit_scope_policy, load_active_commit_scope_policy,
};
pub use context_compactness::{
    ContextBuildCompactness, ContextBuildTelemetry, ContextCompactnessReport,
    ContextComponentCompactness, ContextComponentTelemetry, derive_context_compactness,
    load_context_compactness, record_context_build,
};
pub use context_inspector::{
    ContextInspectorReport, derive_context_inspector, load_context_inspector,
};
pub use context_manifest::{
    CONTEXT_MANIFEST_FILE, CONTEXT_MANIFEST_VERSION, ContextChangeCoverage, ContextFileEntry,
    ContextFileEvidenceReport, ContextFileExclusionReason, ContextFileReason, ContextFileSelection,
    ContextFileStatus, ContextManifest, derive_context_file_evidence, load_context_file_evidence,
    read_context_manifest, select_task_scope_files, write_context_manifest,
};
pub use domain::{
    ChangeSet, ChangeSetId, ChangeSetStatus, EngineeringDomainError, EngineeringEventId,
    EngineeringKnowledge, EngineeringKnowledgeId, EvidenceKind, EvidenceRef, Execution,
    ExecutionId, ExecutionStatus, VerificationCheck, VerificationId, VerificationReceipt,
    VerificationStatus, WorkItem, WorkItemId, WorkItemState, WorkerKind, WorkerRef,
};
pub use events::{
    ENGINEERING_EVENT_LEDGER_FILE, EngineeringEvent, EngineeringEventKind, append_event,
    event_ledger_path, read_events,
};
pub use intelligence::{
    AiUsageIntelligence, ChangeIntelligence, CompletionIntelligence, ContextIntelligence,
    EngineeringIntelligence, ExecutionIntelligence, IntelligenceRates, VerificationIntelligence,
    derive_engineering_intelligence, load_engineering_intelligence,
};
pub use knowledge::{
    ENGINEERING_KNOWLEDGE_FILE, ENGINEERING_KNOWLEDGE_VERSION, EngineeringKnowledgeCategory,
    EngineeringKnowledgeContext, EngineeringKnowledgeCounts, EngineeringKnowledgeOrigin,
    EngineeringKnowledgeProposalInput, EngineeringKnowledgeRecord, EngineeringKnowledgeSnapshot,
    EngineeringKnowledgeStatus, EngineeringKnowledgeStore, EngineeringKnowledgeSuggestion,
    accept_active_engineering_knowledge, archive_active_engineering_knowledge,
    capture_active_verified_command, engineering_knowledge_context, engineering_knowledge_path,
    load_active_engineering_knowledge, propose_active_engineering_knowledge,
};
pub use run_evidence::{
    RunCommitEvidence, RunContextEvidence, RunEvidenceSnapshot, RunReviewEvidence,
    RunVerificationEvidence, RunWorkerEvidence, derive_run_evidence, load_active_run_evidence,
    load_active_run_evidence_from_events,
};
pub use work_item_contract::{
    ScopeComplianceReport, ScopeComplianceStatus, WORK_ITEM_CONTRACT_FILE,
    WORK_ITEM_CONTRACT_VERSION, WorkItemContract, WorkItemContractReadiness,
    WorkItemContractSnapshot, WorkItemContractUpdate, contract_path, derive_scope_compliance,
    derive_work_item_contract_snapshot, load_active_work_item_contract,
    load_work_item_contract_snapshot, read_work_item_contract, save_active_work_item_contract,
};
