import { invoke } from "@tauri-apps/api/core";

export type ProjectAiFile = {
  relative_path: string;
  kind: string;
  label: string;
  size_bytes: number;
  preview: string;
  truncated: boolean;
  blocked: boolean;
  importable: boolean;
  secret_findings: string[];
  warnings: string[];
};

export type ProjectAiScanReport = {
  generated_at: string;
  project: string;
  project_path: string;
  files: ProjectAiFile[];
  warnings: string[];
};

export type ProjectAiImportedFile = {
  relative_path: string;
  memory_id: number;
  truncated: boolean;
};

export type ProjectAiSkippedFile = {
  relative_path: string;
  reason: string;
};

export type ProjectAiImportResult = {
  imported: ProjectAiImportedFile[];
  skipped: ProjectAiSkippedFile[];
  warnings: string[];
};

export async function projectAiScan(): Promise<ProjectAiScanReport> {
  return invoke("project_ai_scan");
}

export async function projectAiImport(paths: string[]): Promise<ProjectAiImportResult> {
  return invoke("project_ai_import", { paths });
}
