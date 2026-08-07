//! RepoDesk 2 engineering domain and event substrate.
//!
//! Keep this layer independent from frontend concerns. Legacy task/orchestrator
//! models adapt into these types until later migration slices move call sites.

pub mod algorithmic_profile;
pub mod context_compactness;
pub mod context_inspector;
pub mod context_manifest;
pub mod domain;
pub mod events;
pub mod instrumentation;
pub mod intelligence;
pub mod work_item_contract;

pub use algorithmic_profile::{
    AlgorithmicEvidence, AlgorithmicEvidenceKind, AlgorithmicProfile, AlgorithmicProfileError,
    AlgorithmicProfileReport, AlgorithmicSignals, AlgorithmicSymbolKind, AnalysisConfidence,
    ComplexityClass, ComplexityHint, MAX_ALGORITHM_SOURCE_BYTES, analyze_rust_file,
    analyze_rust_source,
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
pub use work_item_contract::{
    ScopeComplianceReport, ScopeComplianceStatus, WORK_ITEM_CONTRACT_FILE,
    WORK_ITEM_CONTRACT_VERSION, WorkItemContract, WorkItemContractReadiness,
    WorkItemContractSnapshot, WorkItemContractUpdate, contract_path, derive_scope_compliance,
    load_active_work_item_contract, load_work_item_contract_snapshot, read_work_item_contract,
    save_active_work_item_contract,
};
