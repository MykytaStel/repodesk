export const productPrinciples = [
  {
    title: "One task, one workflow",
    body: "RepoDesk should always know the active project, active task, next safe step, and relevant artifacts.",
  },
  {
    title: "AI is a module, not the owner",
    body: "Codex, ChatGPT, Ollama, and other providers are peripherals controlled by the RepoDesk brain.",
  },
  {
    title: "Security before automation",
    body: "The UI only exposes whitelisted Rust commands. No unrestricted shell access from the desktop.",
  },
  {
    title: "Token discipline",
    body: "Smart context and prompts exist to keep paid agents focused and reduce wasted context.",
  },
];

export const artifactKinds = [
  { kind: "smart_context", label: "Smart context" },
  { kind: "prompt_codex", label: "Codex prompt" },
  { kind: "prompt_chatgpt", label: "ChatGPT prompt" },
  { kind: "prompt_review", label: "Review prompt" },
  { kind: "checks_summary", label: "Checks summary" },
  { kind: "context", label: "Full context" },
  { kind: "token_estimate", label: "Token estimate" },
];

export function statusLabel(status: string): string {
  switch (status) {
    case "done":
      return "Done";
    case "current":
      return "Current";
    case "blocked":
      return "Blocked";
    default:
      return status;
  }
}

export function formatBytes(value: number): string {
  if (!value) return "0 B";
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}

export function summarizeCommand(result?: { ok: boolean; stdout: string; stderr: string }): string {
  if (!result) return "No data";
  const text = `${result.stdout || ""}\n${result.stderr || ""}`.trim();
  if (!text) return result.ok ? "OK" : "No output";
  return text.split("\n").slice(0, 8).join("\n");
}
