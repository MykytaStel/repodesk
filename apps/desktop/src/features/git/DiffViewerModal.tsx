import React, { useEffect, useState } from "react";
import { callCommand } from "../../shared/api/queries";
import { DiffView } from "../../shared/ui/SharedComponents";

interface DiffViewerModalProps {
  filePath: string;
  cached?: boolean;
  onClose: () => void;
}

export function DiffViewerModal({ filePath, cached = false, onClose }: DiffViewerModalProps) {
  const [diff, setDiff] = useState<string>("");
  const [loading, setLoading] = useState<boolean>(true);

  useEffect(() => {
    async function fetchDiff() {
      try {
        const result = await callCommand<string>("git_file_diff", { path: filePath, cached });
        setDiff(result || "No diff available or file is binary.");
      } catch (e: any) {
        setDiff(`Error loading diff: ${e.message || String(e)}`);
      } finally {
        setLoading(false);
      }
    }
    void fetchDiff();
  }, [filePath, cached]);

  // Handle escape key
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  return (
    <div className="modal-overlay" onClick={onClose} style={{
      position: "fixed", top: 0, left: 0, right: 0, bottom: 0,
      backgroundColor: "rgba(0,0,0,0.5)", zIndex: 9999,
      display: "flex", justifyContent: "center", alignItems: "center",
      padding: "2rem"
    }}>
      <div className="modal-content panel" onClick={(e) => e.stopPropagation()} style={{
        width: "100%", maxWidth: "1200px", maxHeight: "90vh",
        display: "flex", flexDirection: "column",
        overflow: "hidden"
      }}>
        <div className="panel-title-row">
          <div>
            <p className="eyebrow">Git Diff</p>
            <h2>{filePath}</h2>
          </div>
          <button className="ghost-button" onClick={onClose}>✖ Close</button>
        </div>
        <div className="scroll-area" style={{ flex: 1, padding: "1rem", overflow: "auto" }}>
          {loading ? (
            <p className="muted">Loading diff...</p>
          ) : (
            <DiffView diff={diff} />
          )}
        </div>
      </div>
    </div>
  );
}
