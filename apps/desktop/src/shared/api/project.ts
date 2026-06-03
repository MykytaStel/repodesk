import { invoke } from "@tauri-apps/api/core";

export type ProjectAddInput = {
  name: string;
  path: string;
  project_type: string;
  main_language?: string | null;
};

export type ProjectConfig = {
  name: string;
  path: string;
  project_type: string;
  main_language?: string | null;
  checks: string[];
  context_ignore: string[];
  created_at: string;
  updated_at: string;
};

export async function getActiveProjectConfig(): Promise<ProjectConfig> {
  return invoke("get_active_project_config");
}

export async function saveProjectIgnoreRules(ignoreRules: string[]): Promise<void> {
  return invoke("save_project_ignore_rules", { ignoreRules });
}
