import { useState } from "react";
import { callCommand } from "../../shared/api/queries";
import { copyToClipboard } from "../../shared/utils/helpers";
import { useToast } from "../../shared/ui/Toast";

export const PAID_PROMPT_AGENT: Record<string, string> = {
  prompt_codex: "codex",
  prompt_chatgpt: "chatgpt",
  prompt_review: "gemini",
};

export type PaidGate = {
  agent: string;
  is_paid: boolean;
  is_patch: boolean;
  decision: string;
  reasons: string[];
  recommendations: string[];
};

export function usePrompts() {
  const [artifactKind, setArtifactKind] = useState("prompt_codex");
  const [artifactContent, setArtifactContent] = useState("");
  const [pendingPaid, setPendingPaid] = useState<{ kind: string; gate: PaidGate } | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const { success } = useToast();

  const loadArtifact = async (kind: string, autoCopy = false) => {
    setArtifactKind(kind);
    setIsLoading(true);
    try {
      const result =
        kind === "agent_context_pack"
          ? await callCommand<any>("agent_context_pack")
          : await callCommand<any>("read_artifact", { kind });
      const content = result?.content || (typeof result === "string" ? result : JSON.stringify(result, null, 2));
      setArtifactContent(content);
      if (autoCopy && content) {
        await copyToClipboard(content);
        success(kind === "agent_context_pack" ? "Copied context pack to clipboard!" : `Copied ${kind.replace("prompt_", "")} prompt to clipboard!`);
      }
    } catch (error: any) {
      setArtifactContent(error?.message || String(error));
    } finally {
      setIsLoading(false);
    }
  };

  const requestPrompt = async (kind: string, autoCopy = false) => {
    const agent = PAID_PROMPT_AGENT[kind];
    if (agent) {
      try {
        const gate = await callCommand<PaidGate>("paid_agent_gate", { agent });
        if (gate?.is_paid) {
          setPendingPaid({ kind, gate });
          setArtifactContent("");
          return;
        }
      } catch {
        // Fallback to load if gate fails
      }
    }
    await loadArtifact(kind, autoCopy);
  };

  const confirmPaidReveal = async () => {
    if (!pendingPaid) return;
    const { kind } = pendingPaid;
    setPendingPaid(null);
    await loadArtifact(kind);
  };

  const cancelPaidReveal = () => setPendingPaid(null);

  const copyPrompt = async () => {
    if (artifactContent) {
      await copyToClipboard(artifactContent);
      success("Copied to clipboard!");
    }
  };

  return {
    artifactKind,
    artifactContent,
    isLoading,
    pendingPaid,
    requestPrompt,
    confirmPaidReveal,
    cancelPaidReveal,
    copyPrompt,
  };
}
