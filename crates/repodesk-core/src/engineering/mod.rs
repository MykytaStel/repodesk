//! RepoDesk 2 engineering domain and event substrate.
//!
//! Keep this layer independent from frontend concerns. Legacy task/orchestrator
//! models adapt into these types until later migration slices move call sites.

pub mod acceptance_evidence;
pub mod ai_strategy;
pub mod ai_usage_intelligence;
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
pub mod knowledge_lifecycle;
pub mod run_evidence;
pub mod run_observability;
pub mod strategy_adaptation;
pub mod strategy_feedback;
pub mod work_item_contract;

pub use acceptance_evidence::{
    ACCEPTANCE_EVIDENCE_FILE, ACCEPTANCE_EVIDENCE_VERSION, AcceptanceCriterionEvidence,
    AcceptanceCriterionStatus, AcceptanceEvidenceBinding, AcceptanceEvidenceReport,
    AcceptanceEvidenceStore, acceptance_evidence_path, active_verification_is_fresh, criterion_id,
    derive_acceptance_evidence, link_active_acceptance_evidence, load_active_acceptance_evidence,
    read_acceptance_evidence,
};
pub use ai_strategy::{
    AiPlanShape, AiStrategyInputs, AiStrategyMode, AiStrategyProfile, AiStrategyReason,
    AiStrategyReasonCode, AiStrategyRecommendation, derive_ai_strategy,
};
pub use ai_usage_intelligence::{
    AiContextEfficiency, AiOrchestrationEfficiency, AiOutcomeEfficiency, AiUsageReport,
    AiUsageSignal, AiUsageSignalCode, AiUsageSignalSeverity, derive_ai_usage_report,
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
    reconfirm_active_engineering_knowledge,
};
pub use knowledge_lifecycle::{
    EngineeringKnowledgeLifecycleCounts, EngineeringKnowledgeLifecycleEntry,
    EngineeringKnowledgeLifecyclePolicy, EngineeringKnowledgeLifecycleReport,
    EngineeringKnowledgeLifecycleState, assess_engineering_knowledge_at,
    derive_engineering_knowledge_lifecycle, derive_engineering_knowledge_lifecycle_at,
    engineering_knowledge_context_eligible, engineering_knowledge_context_eligible_at,
    engineering_knowledge_lifecycle_policy,
};
pub use run_evidence::{
    RunCommitEvidence, RunContextEvidence, RunEvidenceSnapshot, RunReviewEvidence,
    RunVerificationEvidence, RunWorkerEvidence, derive_run_evidence, load_active_run_evidence,
    load_active_run_evidence_from_events,
};
pub use run_observability::{
    RunContextObservability, RunDisposition, RunDispositionStage, RunDispositionState,
    RunEfficiency, RunObservabilityReport, RunStrategyObservability, derive_run_observability,
};
pub use strategy_adaptation::derive_ai_strategy_with_feedback;
pub use strategy_feedback::{
    STRATEGY_FEEDBACK_MIN_SETTLED_RUNS, StrategyFeedbackReport, StrategyOutcomeState,
    StrategyProfileFeedback, StrategyRunFeedback, derive_strategy_feedback,
};
pub use work_item_contract::{
    ScopeComplianceReport, ScopeComplianceStatus, WORK_ITEM_CONTRACT_FILE,
    WORK_ITEM_CONTRACT_VERSION, WorkItemContract, WorkItemContractReadiness,
    WorkItemContractSnapshot, WorkItemContractUpdate, contract_path, derive_scope_compliance,
    derive_work_item_contract_snapshot, load_active_work_item_contract,
    load_work_item_contract_snapshot, read_work_item_contract, save_active_work_item_contract,
};
