import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { projectUse } from "../shared/api/workflow";
import { useToast } from "../shared/ui/Toast";

type ProjectConfig = { name: string; path?: string; project_type?: string };

/**
 * Header control to switch the active project (or jump to connect a new one).
 * Switching re-scopes everything downstream, so it invalidates all queries.
 */
export function ProjectSwitcher({ projectName, onConnectProject }: { projectName: string; onConnectProject: () => void }) {
  const queryClient = useQueryClient();
  const toast = useToast();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  const { data: projects = [] } = useQuery({
    queryKey: ["project_list_configs"],
    queryFn: () => invoke<ProjectConfig[]>("project_list_configs").catch(() => [] as ProjectConfig[]),
    // The connected-projects list only changes on add/switch (which invalidate
    // all queries), so it can stay fresh much longer than the 5s default.
    staleTime: 60_000,
  });

  const switchProject = useMutation({
    mutationFn: (name: string) => projectUse(name),
    onSuccess: (result, name) => {
      if (!result.ok) {
        toast.error(result.stderr || "Could not switch project");
        return;
      }
      toast.success(`Switched to ${name}`);
      setOpen(false);
      void queryClient.invalidateQueries();
    },
    onError: (e: any) => toast.error(e?.message || "Could not switch project"),
  });

  // Close on outside click.
  useEffect(() => {
    if (!open) return;
    const onClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    window.addEventListener("mousedown", onClick);
    return () => window.removeEventListener("mousedown", onClick);
  }, [open]);

  return (
    <div className="project-switcher" ref={ref}>
      <button className="project-switcher-trigger" onClick={() => setOpen((o) => !o)}>
        <h2>{projectName}</h2>
        <span className="project-switcher-caret">▾</span>
      </button>
      {open && (
        <div className="project-switcher-menu">
          {projects.length === 0 && <p className="muted project-switcher-empty">No projects connected.</p>}
          {projects.map((p) => (
            <button
              key={p.name}
              className={`project-switcher-item ${p.name === projectName ? "active" : ""}`}
              disabled={switchProject.isPending || p.name === projectName}
              onClick={() => switchProject.mutate(p.name)}
              title={p.path}
            >
              <span className="project-switcher-check">{p.name === projectName ? "✓" : ""}</span>
              <span className="project-switcher-name">{p.name}</span>
              {p.project_type && <span className="project-switcher-type">{p.project_type}</span>}
            </button>
          ))}
          <div className="project-switcher-sep" />
          <button className="project-switcher-item" onClick={() => { setOpen(false); onConnectProject(); }}>
            <span className="project-switcher-check">+</span>
            <span className="project-switcher-name">Connect project…</span>
          </button>
        </div>
      )}
    </div>
  );
}
