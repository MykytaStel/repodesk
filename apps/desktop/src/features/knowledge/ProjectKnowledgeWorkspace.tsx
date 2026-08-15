import { KnowledgeTab } from "./KnowledgeTab";
import { ProjectAiImportPanel } from "./ProjectAiImportPanel";
import { ProjectGuidelinesPanel } from "./ProjectGuidelinesPanel";

export function ProjectKnowledgeWorkspace() {
  return (
    <div className="project-knowledge-surface">
      <KnowledgeTab />
      <div className="content-grid project-knowledge-inputs">
        <ProjectAiImportPanel />
        <ProjectGuidelinesPanel />
      </div>
    </div>
  );
}
