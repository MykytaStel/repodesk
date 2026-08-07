import { invoke } from "@tauri-apps/api/core";

export type TerminalSessionInfo = {
  session_id: string;
  cwd: string;
  pid: number | null;
  shell: string;
};

export type TerminalOutputPayload = {
  session_id: string;
  sequence: number;
  data: string;
};

export type TerminalExitPayload = {
  session_id: string;
  exit_code: number | null;
  signal: string | null;
  error: string | null;
};

export async function createTerminal(rows: number, cols: number): Promise<TerminalSessionInfo> {
  return invoke("terminal_create", { rows, cols });
}

export async function listTerminals(): Promise<TerminalSessionInfo[]> {
  return invoke("terminal_list");
}

export async function writeTerminal(sessionId: string, data: string): Promise<void> {
  return invoke("terminal_write", { sessionId, data });
}

export async function resizeTerminal(sessionId: string, rows: number, cols: number): Promise<void> {
  return invoke("terminal_resize", { sessionId, rows, cols });
}

export async function killTerminal(sessionId: string): Promise<void> {
  return invoke("terminal_kill", { sessionId });
}
