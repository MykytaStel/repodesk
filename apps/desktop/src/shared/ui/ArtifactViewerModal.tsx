import React, { useEffect, useState } from "react";
import { callCommand } from "../../shared/api/queries";
import { formatNumber } from "../../shared/utils/helpers";

interface ArtifactContent {
  kind: string;
  title: string;
  path: string;
  exists: boolean;
  content: string;
  size_bytes: number;
}

interface ArtifactViewerModalProps {
  kind: string;
  isOpen: boolean;
  onClose: () => void;
}

export function ArtifactViewerModal({ kind, isOpen, onClose }: ArtifactViewerModalProps) {
  const [data, setData] = useState<ArtifactContent | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) return;

    let mounted = true;
    setLoading(true);
    setError(null);

    callCommand<ArtifactContent>("read_artifact", { kind })
      .then((res) => {
        if (mounted) setData(res);
      })
      .catch((err) => {
        if (mounted) setError(err?.message || String(err));
      })
      .finally(() => {
        if (mounted) setLoading(false);
      });

    return () => {
      mounted = false;
    };
  }, [isOpen, kind]);

  if (!isOpen) return null;

  return (
    <div className="modal-backdrop" onClick={onClose} style={{
      position: "fixed",
      top: 0, left: 0, right: 0, bottom: 0,
      backgroundColor: "rgba(0,0,0,0.5)",
      display: "flex",
      alignItems: "center",
      justifyContent: "center",
      zIndex: 1000
    }}>
      <div className="modal-content panel" onClick={(e) => e.stopPropagation()} style={{
        width: "90%",
        maxWidth: "900px",
        maxHeight: "90vh",
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
        backgroundColor: "var(--bg-panel)",
        boxShadow: "0 4px 24px rgba(0,0,0,0.2)"
      }}>
        <div className="panel-title-row" style={{ padding: "16px", borderBottom: "1px solid var(--border)", flexShrink: 0 }}>
          <div>
            <p className="eyebrow">Artifact Viewer</p>
            <h2>{data?.title || "Loading..."}</h2>
            {data && <p className="muted" style={{ fontSize: 13, marginTop: 4 }}>
              {data.path} &bull; {formatNumber(data.size_bytes)} bytes
            </p>}
          </div>
          <button className="ghost-button" onClick={onClose}>Close</button>
        </div>
        
        <div className="modal-body" style={{ padding: "16px", overflowY: "auto", flexGrow: 1 }}>
          {loading && <p className="muted">Loading artifact content...</p>}
          {error && <p className="text-danger">Failed to load artifact: {error}</p>}
          {!loading && !error && data && (
            data.exists ? (
              <pre style={{
                backgroundColor: "var(--bg-body)",
                padding: "16px",
                borderRadius: "6px",
                whiteSpace: "pre-wrap",
                wordWrap: "break-word",
                fontFamily: "var(--font-mono)",
                fontSize: "13px",
                margin: 0
              }}>
                {data.content || "Empty file."}
              </pre>
            ) : (
              <p className="muted">File does not exist yet. Please generate it first.</p>
            )
          )}
        </div>
      </div>
    </div>
  );
}
