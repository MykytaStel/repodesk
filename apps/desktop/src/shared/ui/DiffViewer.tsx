import React, { useMemo } from "react";

interface DiffViewerProps {
  diff: string;
}

export function DiffViewer({ diff }: DiffViewerProps) {
  const lines = useMemo(() => {
    if (!diff.trim()) return [];
    return diff.split("\n");
  }, [diff]);

  if (lines.length === 0) {
    return <div className="muted p-4">No differences found or file is clean.</div>;
  }

  return (
    <div className="code-panel scrollable" style={{ fontFamily: "monospace", fontSize: "0.85em", whiteSpace: "pre-wrap", overflowX: "auto" }}>
      {lines.map((line, i) => {
        let bgColor = "transparent";
        let color = "inherit";

        if (line.startsWith("+") && !line.startsWith("+++")) {
          bgColor = "rgba(46, 160, 67, 0.15)";
          color = "var(--text-ok, #3fb950)";
        } else if (line.startsWith("-") && !line.startsWith("---")) {
          bgColor = "rgba(248, 81, 73, 0.15)";
          color = "var(--text-danger, #f85149)";
        } else if (line.startsWith("@@")) {
          bgColor = "rgba(56, 139, 253, 0.1)";
          color = "var(--text-neutral, #58a6ff)";
        }

        return (
          <div key={i} style={{ backgroundColor: bgColor, color: color, padding: "0 8px" }}>
            {line || " "}
          </div>
        );
      })}
    </div>
  );
}
