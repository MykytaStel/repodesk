import { invoke } from "@tauri-apps/api/core";

export const ENGINEERING_KNOWLEDGE_KEY = ["engineering-knowledge"] as const;

export type EngineeringKnowledgeCategory =
  | "architecture"
  | "convention"
  | "hazard"
  | "command"
  | "testing"
  | "decision"
  | "performance"
  | "tooling";

export type EngineeringKnowledgeStatus = "candidate" | "accepted" | "archived";
export type EngineeringKnowledgeOrigin = "human" | "verification";
export type EngineeringKnowledgeLifecycleState =
  | "pending_review"
  | "current"
  | "review_soon"
  | "review_required"
  | "archived";

export type KnowledgeEvidenceRef = {
  kind: string;
  locator: string;
};

export type EngineeringKnowledgeRecord = {
  id: string;
  project: string;
  category: EngineeringKnowledgeCategory;
  title: string;
  content: string;
  status: EngineeringKnowledgeStatus;
  origin: EngineeringKnowledgeOrigin;
  source_work_item_id: string | null;
  evidence: KnowledgeEvidenceRef[];
  created_at: string;
  updated_at: string;
};

export type EngineeringKnowledgeCounts = {
  candidates: number;
  accepted: number;
  archived: number;
};

export type EngineeringKnowledgeSuggestion = {
  suggestion_id: string;
  category: EngineeringKnowledgeCategory;
  title: string;
  content: string;
  source_work_item_id: string;
  evidence: KnowledgeEvidenceRef[];
};

export type EngineeringKnowledgeLifecycleEntry = {
  knowledge_id: string;
  state: EngineeringKnowledgeLifecycleState;
  age_days: number;
  review_after_days: number | null;
  review_due_at: string | null;
  reason: string;
};

export type EngineeringKnowledgeLifecycleReport = {
  project: string;
  generated_at: string;
  counts: {
    pending_review: number;
    current: number;
    review_soon: number;
    review_required: number;
    archived: number;
  };
  entries: EngineeringKnowledgeLifecycleEntry[];
};

export type EngineeringKnowledgeSnapshot = {
  project: string;
  records: EngineeringKnowledgeRecord[];
  counts: EngineeringKnowledgeCounts;
  suggestions: EngineeringKnowledgeSuggestion[];
  lifecycle: EngineeringKnowledgeLifecycleReport;
};

export type EngineeringKnowledgeProposalInput = {
  category: EngineeringKnowledgeCategory;
  title: string;
  content: string;
};

type KnowledgeAction =
  | { kind: "propose"; input: EngineeringKnowledgeProposalInput }
  | { kind: "capture_command"; command: string }
  | { kind: "accept"; knowledge_id: string }
  | { kind: "reconfirm"; knowledge_id: string }
  | { kind: "archive"; knowledge_id: string };

type KnowledgeTransportResponse = {
  knowledge: Omit<EngineeringKnowledgeSnapshot, "lifecycle"> | null;
  knowledge_lifecycle: EngineeringKnowledgeLifecycleReport | null;
};

async function invokeKnowledge(action?: KnowledgeAction): Promise<EngineeringKnowledgeSnapshot> {
  const response = await invoke<KnowledgeTransportResponse>("work_engineering_intelligence", {
    contractUpdate: null,
    scopeOverrideReason: null,
    runEvidenceId: null,
    acceptanceCriterionId: null,
    acceptanceCommand: null,
    includeKnowledge: true,
    knowledgeAction: action ?? null,
  });
  if (!response.knowledge) throw new Error("Project Engineering Knowledge is unavailable");
  if (!response.knowledge_lifecycle) throw new Error("Project Engineering Knowledge lifecycle is unavailable");
  return { ...response.knowledge, lifecycle: response.knowledge_lifecycle };
}

export async function engineeringKnowledgeSnapshot(): Promise<EngineeringKnowledgeSnapshot> {
  return invokeKnowledge();
}

export async function proposeEngineeringKnowledge(
  input: EngineeringKnowledgeProposalInput,
): Promise<EngineeringKnowledgeSnapshot> {
  return invokeKnowledge({ kind: "propose", input });
}

export async function captureVerifiedKnowledgeCommand(
  command: string,
): Promise<EngineeringKnowledgeSnapshot> {
  return invokeKnowledge({ kind: "capture_command", command });
}

export async function acceptEngineeringKnowledge(
  knowledgeId: string,
): Promise<EngineeringKnowledgeSnapshot> {
  return invokeKnowledge({ kind: "accept", knowledge_id: knowledgeId });
}

export async function reconfirmEngineeringKnowledge(
  knowledgeId: string,
): Promise<EngineeringKnowledgeSnapshot> {
  return invokeKnowledge({ kind: "reconfirm", knowledge_id: knowledgeId });
}

export async function archiveEngineeringKnowledge(
  knowledgeId: string,
): Promise<EngineeringKnowledgeSnapshot> {
  return invokeKnowledge({ kind: "archive", knowledge_id: knowledgeId });
}
