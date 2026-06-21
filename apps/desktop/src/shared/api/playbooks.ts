import { invoke } from "@tauri-apps/api/core";

// A playbook is a named shortcut that opens the surface owning a piece of work.
// Backed by playbooks.toml in core; never starts an agent on its own.
export type Playbook = {
  id: string;
  title: string;
  summary: string;
  target: string;
  destination: string;
  action: string;
  artifact: string;
  starts_agent: boolean;
};

export async function listPlaybooks(): Promise<Playbook[]> {
  return invoke("playbooks_list");
}

export async function savePlaybook(playbook: Playbook): Promise<Playbook[]> {
  return invoke("playbooks_save", { playbook });
}

export async function deletePlaybook(id: string): Promise<Playbook[]> {
  return invoke("playbooks_delete", { id });
}

export async function importPlaybooks(document: string): Promise<Playbook[]> {
  return invoke("playbooks_import", { document });
}
