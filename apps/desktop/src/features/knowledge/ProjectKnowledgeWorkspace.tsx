import { lazy, Suspense } from "react";
import { useWorkspace } from "../../shared/hooks/useWorkspace";
import { ProjectAiImportPanel } from "./ProjectAiImportPanel";
import { ProjectGuidelinesPanel } from "./ProjectGuidelinesPanel";

const KnowledgeTab = lazy(() => import("./KnowledgeTab").then((module) => ({ default: module.KnowledgeTab })));

export function ProjectKnowledgeWorkspace() {
  const { hasProject } = useWorkspace();

  return (
    <div className="project-knowledge-surface">
      <Suspense fallback={<p className="muted">Loading reviewed Engineering Knowledge…</p>}>
        <KnowledgeTab />
      </Suspense>
      {hasProject ? (
        <div className="content-grid project-knowledge-inputs">
          <ProjectAiImportPanel />
          <ProjectGuidelinesPanel />
        </div>
      ) : null}
    </div>
  );
}
