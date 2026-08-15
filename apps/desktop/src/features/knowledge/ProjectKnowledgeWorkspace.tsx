import { useWorkspace } from "../../shared/hooks/useWorkspace";
import { KnowledgeTab } from "./KnowledgeTab";
import { ProjectAiImportPanel } from "./ProjectAiImportPanel";
import { ProjectGuidelinesPanel } from "./ProjectGuidelinesPanel";

export function ProjectKnowledgeWorkspace() {
  const { hasProject } = useWorkspace();

  return (
    <div className="project-knowledge-surface">
      <KnowledgeTab />
      {hasProject ? (
        <div className="content-grid project-knowledge-inputs">
          <ProjectAiImportPanel />
          <ProjectGuidelinesPanel />
        </div>
      ) : null}
    </div>
  );
}
