import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { callCommand, queryKeys } from "../../shared/api/queries";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import { normalizeError } from "../../shared/utils/errors";

interface ProjectGuidelineEntry {
  id: string | number;
  timestamp: string;
  category: string;
  content: string;
}

export function ProjectGuidelinesPanel() {
  const queryClient = useQueryClient();
  const { projectName, hasProject } = useWorkspace();
  const [draft, setDraft] = useState("");

  const guidelines = useQuery({
    queryKey: queryKeys.memory.list(projectName),
    queryFn: async () => {
      const result = await callCommand<unknown>("memory_list", { project: projectName });
      return Array.isArray(result) ? result as ProjectGuidelineEntry[] : [];
    },
    enabled: hasProject,
  });

  const append = useMutation({
    mutationFn: async (content: string) => {
      await callCommand("memory_add", {
        project: projectName,
        content,
        category: "general",
        tags: [],
      });
    },
    onSuccess: () => {
      setDraft("");
      queryClient.invalidateQueries({ queryKey: queryKeys.memory.list(projectName) });
      queryClient.invalidateQueries({ queryKey: queryKeys.memory.preview(projectName) });
    },
  });

  const entries = guidelines.data ?? [];
  const retrievalError = guidelines.isError ? normalizeError(guidelines.error) : null;
  const appendError = append.isError ? normalizeError(append.error) : null;

  return (
    <section className="panel wide-panel project-guidelines-panel">
      <div className="panel-title-row">
        <div>
          <p className="eyebrow">Project guidelines</p>
          <h2>Compatibility instructions</h2>
        </div>
        <button className="tiny-button" type="button" onClick={() => void guidelines.refetch()} disabled={guidelines.isFetching}>
          {guidelines.isFetching ? "Reloading…" : "Reload guidelines"}
        </button>
      </div>
      <p className="muted" style={{ marginBottom: "12px" }}>
        Legacy project guidance used as an input source for agent context. These entries are not reviewed Engineering Knowledge;
        promote durable rules and decisions through the reviewed knowledge lifecycle above.
      </p>

      {retrievalError ? (
        <div className="notice danger" role="alert">
          <strong>Could not load project guidelines.</strong> {retrievalError.message}
        </div>
      ) : guidelines.isLoading ? (
        <p className="muted">Loading compatibility guidelines…</p>
      ) : entries.length === 0 ? (
        <div className="workspace-empty-state">
          <strong>No compatibility guidelines saved yet.</strong>
          <span>Use this only for project-scoped compatibility guidance that has not entered the reviewed knowledge lifecycle.</span>
        </div>
      ) : (
        <div className="code-panel compact" style={{ maxHeight: "250px", marginBottom: "14px", overflowY: "auto", display: "flex", flexDirection: "column", gap: "8px" }}>
          {entries.map((entry) => (
            <div key={entry.id} style={{ borderBottom: "1px solid rgba(255,255,255,0.1)", paddingBottom: "8px" }}>
              <div style={{ fontSize: "0.8em", color: "var(--muted)", marginBottom: "4px" }}>
                {new Date(entry.timestamp).toLocaleString()} <span className="pill neutral" style={{ marginLeft: "8px" }}>{entry.category}</span>
              </div>
              <div style={{ whiteSpace: "pre-wrap" }}>{entry.content}</div>
            </div>
          ))}
        </div>
      )}

      <div className="form-stack" style={{ marginTop: "12px" }}>
        <label>
          Add project compatibility guideline
          <textarea
            rows={3}
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            placeholder="Temporary compatibility guidance or project-specific constraints for agent context…"
          />
        </label>
        <button
          className="primary-button"
          type="button"
          onClick={() => void append.mutateAsync(draft.trim()).catch(() => undefined)}
          disabled={append.isPending || !draft.trim()}
        >
          {append.isPending ? "Adding…" : "Add project guideline"}
        </button>
        {appendError ? (
          <div className="notice danger" role="alert">
            <strong>Could not add project guideline.</strong> {appendError.message}
          </div>
        ) : null}
      </div>
    </section>
  );
}
