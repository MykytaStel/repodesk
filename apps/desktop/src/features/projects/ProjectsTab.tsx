import { lazy, Suspense, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import type { TabId } from "../../shared/types/api";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import "../../shared/ui/secondary-subnav.css";
import "./projects-route.css";

const KnowledgeTab = lazy(() => import("../knowledge/KnowledgeTab").then((module) => ({ default: module.KnowledgeTab })));
const WorkTemplatesTab = lazy(() => import("../playbooks/PlaybooksTab").then((module) => ({ default: module.PlaybooksTab })));

type ProjectsView = "registry" | "knowledge" | "templates";

interface ProjectConfigSummary {
  name: string;
  path: string;
  project_type?: string;
  main_language?: string | null;
  checks?: string[];
  context_ignore?: string[];
}

const VIEWS: Array<{ id: ProjectsView; label: string }> = [
  { id: "registry", label: "Registry" },
  { id: "knowledge", label: "Knowledge" },
  { id: "templates", label: "Work templates" },
];

export function ProjectsTab({ setActiveTab }: { setActiveTab: (tab: TabId, detail?: string) => void }) {
  const [view, setView] = useState<ProjectsView>("registry");
  const queryClient = useQueryClient();
  const { projectName } = useWorkspace();
  const projects = useQuery({
    queryKey: ["project_list_configs"],
    queryFn: () => invoke<ProjectConfigSummary[]>("project_list_configs"),
    enabled: view === "registry",
  });

  const useProject = async (name: string) => {
    await invoke("project_use", { name });
    await queryClient.invalidateQueries();
  };

  return (
    <div className="subnav-host projects-tab">
      <div className="changes-summary">
        <div>
          <p className="eyebrow">Projects</p>
          <strong>Durable repository rules, knowledge and reusable work setup</strong>
        </div>
        {projectName ? <span className="pill accent">Active · {projectName}</span> : <span className="pill neutral">No active project</span>}
      </div>

      <div className="subnav" role="tablist" aria-label="Project views">
        {VIEWS.map((item) => (
          <button
            key={item.id}
            type="button"
            role="tab"
            aria-selected={view === item.id}
            className={view === item.id ? "selected" : ""}
            onClick={() => setView(item.id)}
          >
            {item.label}
          </button>
        ))}
      </div>

      <Suspense fallback={<p className="muted">Loading project capability…</p>}>
        {view === "knowledge" ? <KnowledgeTab /> : null}
        {view === "templates" ? <WorkTemplatesTab setActiveTab={setActiveTab} /> : null}
        {view === "registry" ? (
          <div className="content-grid">
            <section className="hero-panel wide-panel">
              <p className="eyebrow">Project registry</p>
              <h1>Repository workspaces</h1>
              <p className="lead">
                A Project is the durable boundary around repositories, Work Items, checks, context rules and reviewed engineering knowledge.
              </p>
              <div className="button-row">
                <button className="primary-button" type="button" onClick={() => setActiveTab("settings", "Add or configure a project.")}>Add or configure project</button>
                <button className="ghost-button" type="button" onClick={() => void projects.refetch()}>Refresh registry</button>
              </div>
            </section>

            <section className="panel wide-panel">
              <div className="panel-title-row">
                <div>
                  <p className="eyebrow">Connected repositories</p>
                  <h2>{projects.data?.length ?? 0} registered</h2>
                </div>
              </div>

              {projects.isLoading ? (
                <p className="muted">Loading projects…</p>
              ) : projects.isError ? (
                <p className="notice danger">Could not load project registry: {String(projects.error)}</p>
              ) : (projects.data?.length ?? 0) === 0 ? (
                <div className="workspace-empty-state">
                  <strong>No projects registered.</strong>
                  <span>Use Settings to connect a repository. RepoDesk keeps project-specific context and rules bounded to it.</span>
                </div>
              ) : (
                <div className="project-registry-grid">
                  {projects.data?.map((project) => {
                    const active = project.name === projectName;
                    return (
                      <article key={project.name} className={`project-registry-card${active ? " active" : ""}`}>
                        <div className="project-registry-head">
                          <div>
                            <span className="eyebrow">{project.project_type || "repository"}</span>
                            <h3>{project.name}</h3>
                          </div>
                          {active ? <span className="pill ok">Active</span> : null}
                        </div>
                        <code>{project.path}</code>
                        <div className="project-registry-meta">
                          <span>{project.main_language || "language unknown"}</span>
                          <span>{project.checks?.length ?? 0} checks</span>
                          <span>{project.context_ignore?.length ?? 0} context rules</span>
                        </div>
                        <div className="button-row">
                          <button className={active ? "ghost-button" : "primary-button"} type="button" disabled={active} onClick={() => void useProject(project.name)}>
                            {active ? "Current project" : "Open project"}
                          </button>
                          <button className="ghost-button" type="button" onClick={() => setActiveTab("settings", `Configure ${project.name}.`)}>Configure</button>
                          {active ? <button className="ghost-button" type="button" onClick={() => setView("knowledge")}>Knowledge</button> : null}
                        </div>
                      </article>
                    );
                  })}
                </div>
              )}
            </section>
          </div>
        ) : null}
      </Suspense>
    </div>
  );
}
