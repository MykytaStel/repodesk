import { useState, useEffect } from "react";
import { debugEmitter, DebugEventDetail, callCommand } from "../../shared/api/queries";

export function useDebug() {
  const [debugEvents, setDebugEvents] = useState<DebugEventDetail[]>([]);
  const [artifactKind, setArtifactKind] = useState("smart_context");
  const [artifactContent, setArtifactContent] = useState("");

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

  return {
    debugEvents,
    artifactKind,
    artifactContent,
    loadArtifact,
  };
}
