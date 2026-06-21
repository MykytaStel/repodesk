import { useEffect, useMemo, useRef, useState } from "react";
import { formatNumber, stringifyPreview, ActorBadge } from "../../shared/ui/SharedComponents";
import { DiffViewer } from "../../shared/ui/DiffViewer";
import { useCode } from "./useCode";
import { callCommand } from "../../shared/api/queries";
import { FileFindings } from "../../shared/api/repopilot";
import { FindingRow, HealthTrend } from "./CodeFindings";

export function CodeTab() {
  const { changedFiles, report, fileFindings, reviewing, runReview, trend } = useCode();
  const [selectedFile, setSelectedFile] = useState("");
  const [selectedFileContent, setSelectedFileContent] = useState("");
  const autoRan = useRef(false);

  // Auto-run a review once when the tab opens with changes to inspect.
  useEffect(() => {
    if (!autoRan.current && changedFiles.length > 0 && !report && !reviewing) {
      autoRan.current = true;
      runReview();
    }
  }, [changedFiles.length, report, reviewing, runReview]);

  const findingsByFile = useMemo(() => {
    const map = new Map<string, FileFindings>();
    for (const group of fileFindings) map.set(group.file, group);
    return map;
  }, [fileFindings]);

  // Files with findings that aren't in the changed-file list still deserve a row.
  const rows = useMemo(() => {
    const set = new Set(changedFiles);
    for (const group of fileFindings) set.add(group.file);
    return [...set].sort((a, b) => (findingsByFile.get(b)?.worstRank ?? 0) - (findingsByFile.get(a)?.worstRank ?? 0) || a.localeCompare(b));
  }, [changedFiles, fileFindings, findingsByFile]);

  const [viewMode, setViewMode] = useState<"diff" | "file">("diff");

  const loadCodeFile = async (path: string, mode: "diff" | "file" = viewMode) => {
    setSelectedFile(path);
    setSelectedFileContent("Loading...");
    try {
      if (mode === "diff") {
        const result = await callCommand<string>("git_file_diff", { path, cached: false });
        if (!result.trim()) {
          const cachedResult = await callCommand<string>("git_file_diff", { path, cached: true });
          setSelectedFileContent(cachedResult.trim() ? cachedResult : "No differences found.");
        } else {
          setSelectedFileContent(result);
        }
      } else {
        const result = await callCommand<any>("read_code_file", { relativePath: path, relative_path: path });
        setSelectedFileContent(result?.content ?? stringifyPreview(result));
      }
    } catch (error) {
      setSelectedFileContent(String(error));
    }
  };

  // Re-load when view mode changes
  useEffect(() => {
    if (selectedFile) loadCodeFile(selectedFile, viewMode);
  }, [viewMode]);

  const selectedGroup = selectedFile ? findingsByFile.get(selectedFile) : undefined;
  const counts = report?.counts;

  return (
    <div className="content-grid two-column-grid">
      <div className="left-column">
        <section className="hero-panel">
          <p className="eyebrow">Code workbench</p>
          <h1>{formatNumber(changedFiles.length)} files changed.</h1>
          <p className="lead">Review unstaged and staged edits with inline RepoPilot findings before building context. Commits are not handled here.</p>
          <div className="button-row">
            <button className="primary-button" onClick={() => runReview()} disabled={reviewing}>
              {reviewing ? "Reviewing…" : report ? "Re-run RepoPilot" : "Run RepoPilot"}
            </button>
            <ActorBadge mode="auto" />
          </div>
          {report?.error ? (
            <div className="notice danger" style={{ marginTop: 12 }}>{report.error}</div>
          ) : report ? (
            <div className="route-summary-grid" style={{ marginTop: 12 }}>
              <div><span>Critical</span><strong>{counts?.critical ?? 0}</strong></div>
              <div><span>High</span><strong>{counts?.high ?? 0}</strong></div>
              <div><span>Medium</span><strong>{counts?.medium ?? 0}</strong></div>
              <div><span>Low</span><strong>{counts?.low ?? 0}</strong></div>
            </div>
          ) : null}
          <HealthTrend points={trend} />
        </section>

        <section className="panel">
          <div className="panel-title-row compact">
            <h2>Changed files</h2>
            <span className="pill">{rows.length}</span>
          </div>
          <div className="file-list scroll-area small">
            {rows.length === 0 && <p className="muted">No changed files.</p>}
            {rows.map((file) => {
              const group = findingsByFile.get(file);
              const active = file === selectedFile;
              return (
                <button
                  key={file}
                  className={`file-row ${active ? "active" : ""}`}
                  onClick={() => void loadCodeFile(file)}
                >
                  <code>{file}</code>
                  {group && (
                    <span className="file-badges">
                      {group.blocking > 0 && <span className="pill danger">{group.blocking}</span>}
                      <span className="pill neutral">{group.total}</span>
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        </section>
      </div>

      <div className="right-column">
        <section className="panel">
          <div className="panel-title-row">
            <p className="eyebrow">Findings</p>
            {selectedFile && <strong>{selectedFile}</strong>}
          </div>
          {!report ? (
            <p className="muted">Run RepoPilot to surface findings inline.</p>
          ) : selectedGroup ? (
            <ul className="findings-list">
              {selectedGroup.findings.map((finding, index) => <FindingRow key={index} finding={finding} />)}
            </ul>
          ) : selectedFile ? (
            <p className="muted">No findings in this file.</p>
          ) : fileFindings.length === 0 ? (
            <p className="muted">No findings in the current diff.</p>
          ) : (
            fileFindings.map((group) => (
              <div key={group.file} className="finding-group">
                <div className="finding-group-head">
                  <code>{group.file}</code>
                  {group.blocking > 0 && <span className="pill danger">{group.blocking} blocking</span>}
                </div>
                <ul className="findings-list">
                  {group.findings.map((finding, index) => <FindingRow key={index} finding={finding} />)}
                </ul>
              </div>
            ))
          )}
        </section>

        <section className="panel fill-height preview-panel">
          <div className="panel-title-row">
            <p className="eyebrow">File preview</p>
            {selectedFile && (
              <div style={{ display: "flex", alignItems: "center", gap: "12px" }}>
                <strong>{selectedFile}</strong>
                <div className="button-row" style={{ marginTop: 0 }}>
                  <button className={`tiny-button ${viewMode === "diff" ? "primary-button" : "ghost-button"}`} onClick={() => setViewMode("diff")}>Diff</button>
                  <button className={`tiny-button ${viewMode === "file" ? "primary-button" : "ghost-button"}`} onClick={() => setViewMode("file")}>Full File</button>
                </div>
              </div>
            )}
          </div>
          {viewMode === "diff" ? (
            <DiffViewer diff={selectedFileContent === "Loading..." ? "" : selectedFileContent} />
          ) : (
            <pre className="code-panel scrollable">{selectedFileContent || "Select a file to preview"}</pre>
          )}
        </section>
      </div>
    </div>
  );
}
