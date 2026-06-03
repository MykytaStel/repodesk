export const CATEGORIES = ["general", "decision", "constraint", "context", "risk", "pattern"] as const;
export type Category = (typeof CATEGORIES)[number];

export type Tone = "ok" | "warn" | "danger" | "neutral";
export type EntryStatusFilter = "active" | "archived" | "all";

export const CATEGORY_TONE: Record<string, Tone> = {
  general: "neutral",
  decision: "ok",
  constraint: "warn",
  context: "neutral",
  risk: "danger",
  pattern: "ok",
};

export const AGENTS = ["chatgpt", "codex", "gemini", "ollama", "claude", "human"] as const;

export const KIND_TONE: Record<string, Tone> = {
  capture: "ok",
  dedup: "neutral",
  merge: "warn",
  conflict: "danger",
};
