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

export type EngineeringKnowledgeSnapshot = {
  project: string;
  records: EngineeringKnowledgeRecord[];
  counts: EngineeringKnowledgeCounts;
  suggestions: EngineeringKnowledgeSuggestion[];
};

export type EngineeringKnowledgeProposalInput = {
  category: EngineeringKnowledgeCategory;
  title: string;
  content: string;
};

export async function engineeringKnowledgeSnapshot(): Promise<EngineeringKnowledgeSnapshot> {
  return invoke("engineering_knowledge_snapshot");
}

export async function proposeEngineeringKnowledge(
  input: EngineeringKnowledgeProposalInput,
): Promise<EngineeringKnowledgeSnapshot> {
  return invoke("engineering_knowledge_propose", { input });
}

export async function captureVerifiedKnowledgeCommand(
  command: string,
): Promise<EngineeringKnowledgeSnapshot> {
  return invoke("engineering_knowledge_capture_command", { command });
}

export async function acceptEngineeringKnowledge(
  knowledgeId: string,
): Promise<EngineeringKnowledgeSnapshot> {
  return invoke("engineering_knowledge_accept", { knowledgeId });
}

export async function archiveEngineeringKnowledge(
  knowledgeId: string,
): Promise<EngineeringKnowledgeSnapshot> {
  return invoke("engineering_knowledge_archive", { knowledgeId });
}
