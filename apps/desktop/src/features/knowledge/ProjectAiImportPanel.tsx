import { useMemo, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { projectAiImport, projectAiScan, type ProjectAiFile, type ProjectAiScanReport } from "../../shared/api/projectAi";
import { queryKeys } from "../../shared/api/queries";
import { useToast } from "../../shared/ui/Toast";
import { useWorkspace } from "../../shared/hooks/useWorkspace";

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MB`;
}

function statusFor(file: ProjectAiFile) {
  if (file.blocked) return { tone: "danger", label: "blocked" };
  if (file.importable) return { tone: "ok", label: "importable" };
  return { tone: "neutral", label: "skipped" };
}

export function ProjectAiImportPanel() {
  const queryClient = useQueryClient();
  const toast = useToast();
  const { projectName } = useWorkspace();
  const [report, setReport] = useState<ProjectAiScanReport | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const [expanded, setExpanded] = useState<string | null>(null);

  const selectedSet = useMemo(() => new Set(selected), [selected]);
  const importable = report?.files.filter((file) => file.importable) ?? [];

  const scan = useMutation({
    mutationFn: projectAiScan,
    onSuccess: (data) => {
      setReport(data);
      setSelected(data.files.filter((file) => file.importable).map((file) => file.relative_path));
      queryClient.setQueryData(queryKeys.projectAi.scan(projectName), data);
    },
  });

  const doImport = useMutation({
    mutationFn: (paths: string[]) => projectAiImport(paths),
    onSuccess: (result) => {
      queryClient.invalidateQueries({ queryKey: queryKeys.memory.list(projectName) });
      queryClient.invalidateQueries({ queryKey: queryKeys.memory.preview(projectName) });
      toast.success(`Imported ${result.imported.length} project AI file(s)`);
    },
    onError: (error: any) => {
      toast.error(error?.message || "Could not import project AI files");
    },
  });

  const toggle = (path: string) => {
    setSelected((current) =>
      current.includes(path) ? current.filter((item) => item !== path) : [...current, path],
    );
  };

  return (
    <section className="panel wide-panel project-ai-import">
      <div className="panel-title-row">
        <div>
          <p className="eyebrow">Project inputs</p>
          <h2>Import context from other AI tools</h2>
        </div>
        <div className="button-row">
          <button className="ghost-button" onClick={() => scan.mutate()} disabled={scan.isPending}>
            {scan.isPending ? "Scanning..." : report ? "Re-scan" : "Scan project"}
          </button>
          <button
            className="primary-button"
            onClick={() => doImport.mutate(selected)}
            disabled={doImport.isPending || selected.length === 0}
          >
            {doImport.isPending ? "Importing..." : `Import selected (${selected.length})`}
          </button>
        </div>
      </div>
      <p className="muted">
        Finds AGENTS.md, CLAUDE.md, Cursor/Claude folders, and Copilot instruction files in the active repo.
        Imported files enter project compatibility memory; they are not reviewed Engineering Knowledge until promoted through the review lifecycle.
        Files with secret-like content are blocked before import.
      </p>

      {scan.error && <div className="notice danger" role="alert">{(scan.error as Error).message}</div>}
      {doImport.data && (
        <div className="notice ok" role="status">
          Imported {doImport.data.imported.length} file(s)
          {doImport.data.skipped.length ? `; skipped ${doImport.data.skipped.length}.` : "."}
        </div>
      )}
      {report?.warnings.length ? (
        <div className="notice warn">{report.warnings.join(" ")}</div>
      ) : null}

      {!report ? (
        <p className="muted">No project AI scan has run yet.</p>
      ) : report.files.length === 0 ? (
        <p className="muted">No project AI instruction files were found.</p>
      ) : (
        <div className="table-list">
          {report.files.map((file) => {
            const status = statusFor(file);
            const checked = selectedSet.has(file.relative_path);
            return (
              <div className="table-row flex-col items-start gap-sm" key={file.relative_path}>
                <div className="project-ai-row-main">
                  <label className="project-ai-check">
                    <input
                      type="checkbox"
                      checked={checked}
                      disabled={!file.importable}
                      onChange={() => toggle(file.relative_path)}
                    />
                    <span>
                      <strong>{file.relative_path}</strong>
                      <span>{file.label} · {formatBytes(file.size_bytes)}</span>
                    </span>
                  </label>
                  <div className="row-meta">
                    <span className={`pill ${status.tone}`}>{status.label}</span>
                    <span className="pill neutral">{file.kind}</span>
                    <button
                      className="tiny-button ghost-button"
                      onClick={() => setExpanded(expanded === file.relative_path ? null : file.relative_path)}
                    >
                      {expanded === file.relative_path ? "Hide" : "Preview"}
                    </button>
                  </div>
                </div>
                {file.secret_findings.length > 0 && (
                  <div className="notice danger compact-notice">
                    Secret scan: {file.secret_findings.join(", ")}
                  </div>
                )}
                {file.warnings.length > 0 && (
                  <div className="notice warn compact-notice">{file.warnings.join(" ")}</div>
                )}
                {expanded === file.relative_path && (
                  <pre className="code-panel compact project-ai-preview">
                    {file.preview || "No preview available."}
                    {file.truncated ? "\n\n[Preview truncated]" : ""}
                  </pre>
                )}
              </div>
            );
          })}
        </div>
      )}

      {report && importable.length > 0 && (
        <div className="phase-actions">
          <button
            className="tiny-button ghost-button"
            onClick={() => setSelected(importable.map((file) => file.relative_path))}
          >
            Select all importable
          </button>
          <button className="tiny-button ghost-button" onClick={() => setSelected([])}>
            Clear
          </button>
        </div>
      )}
    </section>
  );
}
