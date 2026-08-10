import { invoke } from "@tauri-apps/api/core";

export const REPOSITORY_INTELLIGENCE_KEY = ["repository", "intelligence-v1"] as const;

export type RepositorySemanticStrategy = "rust_ast" | "script_literal_imports" | "unavailable";
export type RepositoryEvidenceLevel = "strong" | "bounded" | "unavailable";

export type RepositoryLanguageCoverage = {
  language: string;
  visible_files: number;
  semantic_files_indexed: number;
  semantic_bytes_indexed: number;
  strategy: RepositorySemanticStrategy;
  evidence_level: RepositoryEvidenceLevel;
  limitations: string[];
};

export type RepositorySemanticCoverage = {
  semantic_files_eligible: number;
  semantic_files_indexed: number;
  semantic_bytes_indexed: number;
  languages: RepositoryLanguageCoverage[];
};

export type RepositoryGraphEvidence = {
  strategy: RepositorySemanticStrategy;
  level: RepositoryEvidenceLevel;
  indexed: boolean;
  reasons: string[];
  limitations: string[];
};

export type RepositoryRelation = {
  path: string;
  reason: string;
};

export type RepositoryTestCandidate = {
  path: string;
  score: number;
  reason: string;
};

export type RepositoryCoChange = {
  path: string;
  commits_together: number;
  focus_commits_sampled: number;
};

export type RepositoryContextCandidate = {
  path: string;
  score: number;
  reasons: string[];
};

export type RepositoryFileIntelligence = {
  path: string;
  language: string;
  graph_evidence: RepositoryGraphEvidence;
  dependencies: RepositoryRelation[];
  dependents: RepositoryRelation[];
  closest_tests: RepositoryTestCandidate[];
  co_changes: RepositoryCoChange[];
  context_candidates: RepositoryContextCandidate[];
};

export type RepositoryIntelligenceSnapshot = {
  version: number;
  project: string;
  focus_path: string | null;
  indexed_files: number;
  rust_files_indexed: number;
  rust_bytes_indexed: number;
  coverage: RepositorySemanticCoverage;
  truncated: boolean;
  git_history_available: boolean;
  focus: RepositoryFileIntelligence | null;
};

export async function repositoryIntelligenceSnapshot(
  focusPath: string | null,
): Promise<RepositoryIntelligenceSnapshot> {
  return invoke<RepositoryIntelligenceSnapshot>("repository_intelligence_snapshot", {
    focusPath,
  });
}
