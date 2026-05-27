import React from "react";
import { asArray, asRecord, getString, getValue, formatNumber, copyToClipboard } from "../../shared/ui/SharedComponents";

interface CodeTabProps {
  codeWorkbench: any;
  changedFiles: string[];
  isBusy: boolean;
  selectedFile: string;
  selectedFileContent: string;
  runAction: (actionId: string) => void;
  refreshAll: (label: string) => void;
  loadCodeFile: (path: string) => void;
  pushToast: (kind: any, title: string, message?: string) => void;
}

export function CodeTab({
  codeWorkbench,
  changedFiles,
  isBusy,
  selectedFile,
  selectedFileContent,
  runAction,
  refreshAll,
  loadCodeFile,
  pushToast,
}: CodeTabProps) {
  const previews = asArray(asRecord(codeWorkbench).previews);

  return (
    <div className="content-grid code-grid">
      <section className="hero-panel wide-panel">
        <p className="eyebrow">Code</p>
        <h1>{changedFiles.length} changed files visible.</h1>
        <p className="lead">Inspect changed files before building prompts or asking an agent. Secret-like and binary paths stay blocked.</p>
        <div className="button-row">
          <button className="primary-button" onClick={() => void runAction("smart-context-build")} disabled={isBusy}>Build smart context</button>
          <button className="ghost-button" onClick={() => void refreshAll("Refreshing code workbench")} disabled={isBusy}>Refresh code</button>
        </div>
      </section>

      <section className="panel file-browser-panel">
        <div className="panel-title-row"><div><p className="eyebrow">Changed files</p><h2>Review before AI</h2></div><span className="pill">{changedFiles.length}</span></div>
        <div className="file-list scroll-area">
          {changedFiles.length === 0 ? <p className="muted">No changed files found or no active project connected.</p> : changedFiles.map((file) => (
            <button key={file} className={`file-row ${selectedFile === file ? "active" : ""}`} onClick={() => void loadCodeFile(file)}><code>{file}</code></button>
          ))}
        </div>
      </section>

      <section className="panel code-preview-panel">
        <div className="panel-title-row"><div><p className="eyebrow">File preview</p><h2>{selectedFile || "Select a file"}</h2></div>{selectedFileContent && <button className="tiny-button" onClick={() => void copyToClipboard(selectedFileContent).then(() => pushToast("success", "Copied", selectedFile))}>Copy</button>}</div>
        <pre className="code-panel tall">{selectedFileContent || "Pick a changed file to inspect safe preview."}</pre>
      </section>

      <section className="panel wide-panel">
        <div className="panel-title-row"><div><p className="eyebrow">Safe snippets</p><h2>Context candidates</h2></div><span className="pill">{previews.length}</span></div>
        <div className="snippet-grid">
          {previews.length === 0 ? <p className="muted">No previews yet.</p> : previews.slice(0, 8).map((item, index) => {
            const record = asRecord(item);
            return <div className="snippet-card" key={getString(record, "path", String(index))}><strong>{getString(record, "path", `file-${index}`)}</strong><span>{formatNumber(Number(getValue(record, "bytes") ?? 0))} bytes - {getString(record, "status", "changed")}</span></div>;
          })}
        </div>
      </section>
    </div>
  );
}
