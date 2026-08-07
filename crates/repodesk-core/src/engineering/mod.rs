//! RepoDesk 2 engineering domain and event substrate.
//!
//! Keep this layer independent from frontend concerns. Legacy task/orchestrator
//! models adapt into these types until later migration slices move call sites.

pub mod domain;
pub mod events;

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
