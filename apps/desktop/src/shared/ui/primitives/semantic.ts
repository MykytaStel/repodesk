export type SemanticTone = "positive" | "attention" | "critical" | "neutral" | "info";

export type SemanticState = {
  label: string;
  tone: SemanticTone;
  detail?: string;
};
