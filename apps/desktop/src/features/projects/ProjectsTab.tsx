import { lazy, Suspense, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import type { TabId } from "../../shared/types/api";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import { WORK_ENGINEERING_SNAPSHOT_KEY } from "../../shared/api/engineering";
import {
  ActionBar,
  EmptyState,
  ErrorState,
  EvidenceState,
  LoadingState,
  PanelHeader,
  StatusBadge,
} from "../../shared/ui/primitives";
import "../../shared/ui/secondary-subnav.css";
import "./projects-route.css";
import {
  attributionPolicySemantic,
  projectNoticeSemantic,
  projectWorkspaceSemantic,
} from "./projectsSemantic";
import { useProjectSetup } from "./useProjectSetup";

const ProjectKnowledgeWorkspace = lazy(() => import("../knowledge/ProjectKnowledgeWorkspace").then((module) => ({ default: module.ProjectKnowledgeWorkspace })));
const WorkTemplatesTab = lazy(() => import("../playbooks/PlaybooksTab").then((module) => ({ default: module.PlaybooksTab })));

type ProjectsView = "registry" | "knowledge" | "templates";

interface ProjectConfigSummary {
  name: string;
  path: string;
  project_type?: string;
  main_language?: string | null;
  checks?: string[];
  context_ignore?: string[];
  require_exact_change_attribution?: boolean;
}

const VIEWS: Array<{ id: ProjectsView; label: string }> = [
  { id: "registry", label: "Registry" },
  { id: "knowledge", label: "Knowledge" },
  { id: "templates", label: "Work templates" },
];

export function ProjectsTab({ setActiveTab }: { setActiveTab: (tab: TabId, detail?: string) => void }) {
  const [view, setView] = useState<ProjectsView>("registry");
  const [showSetup, setShowSetup] = useState(false);
  const { projectName, hasProject } = useWorkspace();
  const queryClient = useQueryClient();
  const {
    setupForm,
    setSetupForm,
    setupNotice,
    activationNotice,
    browseForProjectPath,
    addProject,
    isAddingProject,
    activateProject,
    isActivatingProject,
    activatingProjectName,
  } = useProjectSetup();
  const projects = useQuery({
    queryKey: ["project_list_configs"],
    queryFn: () => invoke<ProjectConfigSummary[]>("project_list_configs"),
    enabled: view === "registry",
  });
  const attributionPolicy = useMutation({
    mutationFn: ({ name, required }: { name: string; required: boolean }) =>
      invoke<ProjectConfigSummary>("project_set_exact_attribution_required", { name, required }),
    onSuccess: (updated) => {
      queryClient.setQueryData<ProjectConfigSummary[]>(["project_list_configs"], (current) =>
        current?.map((project) => project.name === updated.name ? updated : project),
      );
      if (updated.name === projectName) {
        void queryClient.invalidateQueries({ queryKey: WORK_ENGINEERING_SNAPSHOT_KEY });
      }
    },
  });
  const projectMutationPending = isAddingProject || isActivatingProject || attributionPolicy.isPending;
  const workspaceSemantic = projectWorkspaceSemantic(hasProject ? "active" : "inactive");
  const setupSemantic = setupNotice ? projectNoticeSemantic(setupNotice.tone) : null;
  const activationSemantic = activationNotice ? projectNoticeSemantic(activationNotice.tone) : null;

  return (
    <div className="subnav-host projects-tab">
      <div className="changes-summary">
        <PanelHeader
          eyebrow="Projects"
          title="Durable repository rules, knowledge and reusable work setup"
          description="Project-scoped repository configuration, evidence and reusable engineering context."
          trailing={(
            <StatusBadge
              label={hasProject ? `Active · ${projectName}` : workspaceSemantic.label}
              tone={workspaceSemantic.tone}
            />
          )}
        />
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

      <Suspense fallback={<LoadingState message="Loading project capability…" />}>
        {view === "knowledge" ? <ProjectKnowledgeWorkspace /> : null}
        {view === "templates" ? <WorkTemplatesTab setActiveTab={setActiveTab} /> : null}
        {view === "registry" ? (
          <div className="content-grid">
            <section className="hero-panel wide-panel">
              <PanelHeader
                eyebrow="Project registry"
                title="Repository workspaces"
                description="A Project is the durable boundary around repositories, Work Items, checks, context rules and reviewed engineering knowledge."
              />
              <ActionBar
                primary={(
                  <button className="primary-button" type="button" onClick={() => setShowSetup((visible) => !visible)}>
                    {showSetup ? "Close setup" : "Add project"}
                  </button>
                )}
                secondary={<button className="ghost-button" type="button" onClick={() => void projects.refetch()}>Refresh registry</button>}
              />
            </section>

            {showSetup ? (
              <section className="panel wide-panel project-setup-panel">
                <PanelHeader
                  eyebrow="Connect repository"
                  title="Add and activate a project"
                  trailing={<StatusBadge label="Project-scoped" tone="neutral" />}
                />
                <div className="form-stack">
                  <label>
                    Project name
                    <input
                      value={setupForm.projectName}
                      onChange={(event) => setSetupForm({ ...setupForm, projectName: event.target.value })}
                      placeholder="my-app"
                    />
                  </label>
                  <label>
                    Project path
                    <div className="input-with-action">
                      <input
                        value={setupForm.projectPath}
                        onChange={(event) => setSetupForm({ ...setupForm, projectPath: event.target.value })}
                        placeholder="/Users/you/code/my-app"
                      />
                      <button type="button" className="ghost-button" onClick={() => void browseForProjectPath()}>
                        Browse…
                      </button>
                    </div>
                  </label>
                  <div className="settings-grid">
                    <label>
                      Project type
                      <input
                        value={setupForm.projectType}
                        onChange={(event) => setSetupForm({ ...setupForm, projectType: event.target.value })}
                        placeholder="repository"
                      />
                    </label>
                    <label>
                      Main language
                      <input
                        value={setupForm.mainLanguage}
                        onChange={(event) => setSetupForm({ ...setupForm, mainLanguage: event.target.value })}
                        placeholder="rust, typescript, …"
                      />
                    </label>
                  </div>
                  <ActionBar
                    primary={(
                      <button
                        className="primary-button"
                        type="button"
                        onClick={() => void addProject().catch(() => undefined)}
                        disabled={projectMutationPending}
                      >
                        {isAddingProject ? "Adding and activating…" : "Add and activate project"}
                      </button>
                    )}
                    secondary={(
                      <button className="ghost-button" type="button" onClick={() => setShowSetup(false)} disabled={projectMutationPending}>
                        Cancel
                      </button>
                    )}
                  />
                  {setupNotice && setupSemantic ? (
                    setupNotice.tone === "danger" ? (
                      <ErrorState title="Project setup failed" detail={setupNotice.message} />
                    ) : (
                      <EvidenceState
                        label="Project setup"
                        state={setupSemantic.label}
                        tone={setupSemantic.tone}
                        detail={setupNotice.message}
                        role="status"
                      />
                    )
                  ) : null}
                </div>
              </section>
            ) : null}

            <section className="panel wide-panel">
              <PanelHeader
                eyebrow="Connected repositories"
                title={`${projects.data?.length ?? 0} registered`}
              />

              {activationNotice && activationSemantic ? (
                activationNotice.tone === "danger" ? (
                  <ErrorState title="Project activation failed" detail={activationNotice.message} />
                ) : (
                  <EvidenceState
                    label="Project activation"
                    state={activationSemantic.label}
                    tone={activationSemantic.tone}
                    detail={activationNotice.message}
                    role="status"
                  />
                )
              ) : null}
              {attributionPolicy.isError ? (
                <ErrorState
                  title="Could not update project trust policy"
                  detail={String(attributionPolicy.error)}
                />
              ) : null}

              {projects.isLoading ? (
                <LoadingState message="Loading projects…" />
              ) : projects.isError ? (
                <ErrorState title="Project registry unavailable" detail={String(projects.error)} />
              ) : (projects.data?.length ?? 0) === 0 ? (
                <EmptyState
                  message="No projects registered."
                  hint="Add a repository here. RepoDesk keeps project-specific context and rules bounded to that Project."
                />
              ) : (
                <div className="project-registry-grid">
                  {projects.data?.map((project) => {
                    const active = hasProject && project.name === projectName;
                    const activating = isActivatingProject && activatingProjectName === project.name;
                    const exactRequired = project.require_exact_change_attribution === true;
                    const policySemantic = attributionPolicySemantic(exactRequired);
                    const policyActionLabel = exactRequired ? "Use informational attribution" : "Require exact attribution";
                    const policyDetail = exactRequired
                      ? "Finish requires exact producer attribution for this Project."
                      : "Attribution remains visible evidence but does not block Finish.";
                    const secondaryActions = (
                      <div className="button-row">
                        <button
                          className="ghost-button"
                          type="button"
                          disabled={projectMutationPending}
                          onClick={() => attributionPolicy.mutate({ name: project.name, required: !exactRequired })}
                          aria-pressed={exactRequired}
                          title="When required, Finish blocks any ChangeSet without exact producer attribution."
                        >
                          {policyActionLabel}
                        </button>
                        {active ? <button className="ghost-button" type="button" onClick={() => setView("knowledge")}>Knowledge</button> : null}
                      </div>
                    );

                    return (
                      <article key={project.name} className={`project-registry-card${active ? " active" : ""}`}>
                        <div className="project-registry-head">
                          <div>
                            <span className="eyebrow">{project.project_type || "repository"}</span>
                            <h3>{project.name}</h3>
                          </div>
                          {active ? <StatusBadge label="Active" tone="positive" /> : null}
                        </div>
                        <code>{project.path}</code>
                        <div className="project-registry-meta">
                          <span>{project.main_language || "language unknown"}</span>
                          <span>{project.checks?.length ?? 0} checks</span>
                          <span>{project.context_ignore?.length ?? 0} context rules</span>
                        </div>
                        <EvidenceState
                          label="Producer attribution policy"
                          state={policySemantic.label}
                          tone={policySemantic.tone}
                          detail={policyDetail}
                        />
                        <ActionBar
                          primary={active ? undefined : (
                            <button
                              className="primary-button"
                              type="button"
                              disabled={projectMutationPending}
                              onClick={() => void activateProject(project.name).catch(() => undefined)}
                            >
                              {activating ? "Opening…" : "Open project"}
                            </button>
                          )}
                          secondary={secondaryActions}
                          detail={active ? "Current project" : undefined}
                        />
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
