import { useState, useEffect } from "react";
import { debugEmitter, DebugEventDetail, callCommand } from "../../shared/api/queries";

/// Prompt artifact kinds destined for a paid/cloud agent. Revealing these is a
/// human hand-off, so it must pass the paid-agent safety gate first.
const PAID_PROMPT_AGENT: Record<string, string> = {
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

export function useDebug() {
  const [debugEvents, setDebugEvents] = useState<DebugEventDetail[]>([]);
  const [artifactKind, setArtifactKind] = useState("smart_context");
  const [artifactContent, setArtifactContent] = useState("");
  const [pendingPaid, setPendingPaid] = useState<{ kind: string; gate: PaidGate } | null>(null);

  useEffect(() => {
    const handleDebugEvent = (e: Event) => {
      const customEvent = e as CustomEvent<DebugEventDetail>;
      setDebugEvents((prev) => [customEvent.detail, ...prev].slice(0, 150));
    };

    debugEmitter.addEventListener("debug-command", handleDebugEvent);
    return () => {
      debugEmitter.removeEventListener("debug-command", handleDebugEvent);
    };
  }, []);

  const loadArtifact = async (kind: string) => {
    setArtifactKind(kind);
    setArtifactContent("Loading...");
    try {
      const result = await callCommand<any>("read_artifact", { kind });
      const content = result?.content || (typeof result === "string" ? result : JSON.stringify(result, null, 2));
      setArtifactContent(content);
    } catch (error: any) {
      setArtifactContent(error?.message || String(error));
    }
  };

  // Reveal an artifact, but gate paid/cloud-agent prompts behind a safety check
  // and an explicit confirmation so secrets cannot be copied out unknowingly.
  const requestArtifact = async (kind: string) => {
    const agent = PAID_PROMPT_AGENT[kind];
    if (agent) {
      try {
        const gate = await callCommand<PaidGate>("paid_agent_gate", { agent });
        if (gate?.is_paid) {
          setPendingPaid({ kind, gate });
          return;
        }
      } catch {
        // If the gate is unavailable, fall through to a normal reveal.
      }
    }
    await loadArtifact(kind);
  };

  const confirmPaidReveal = async () => {
    if (!pendingPaid) return;
    const { kind } = pendingPaid;
    setPendingPaid(null);
    await loadArtifact(kind);
  };

  const cancelPaidReveal = () => setPendingPaid(null);

  return {
    debugEvents,
    artifactKind,
    artifactContent,
    requestArtifact,
    pendingPaid,
    confirmPaidReveal,
    cancelPaidReveal,
  };
}
