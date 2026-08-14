import type { TabId, Theme } from "../shared/types/api";
import { ProjectSwitcher } from "./ProjectSwitcher";
import { ThemeMenu } from "./ThemeMenu";
import type { AppTab } from "./tabs";

interface WorkspaceSidebarProps {
  activeTab: TabId;
  activeTabInfo: AppTab;
  projectName: string;
  taskTitle: string;
  hasProject: boolean;
  hasTask: boolean;
  dirty: boolean;
  dirtyCount: number;
  theme: Theme;
  onThemeChange: (theme: Theme) => void;
  onNavigate: (tab: TabId, detail?: string) => void;
}

export function WorkspaceSidebar({
  activeTab,
  activeTabInfo,
  projectName,
  taskTitle,
  hasProject,
  hasTask,
  dirty,
  dirtyCount,
  theme,
  onThemeChange,
  onNavigate,
}: WorkspaceSidebarProps) {
  void activeTab;

  return (
    <aside className="workspace-sidebar" aria-label="Workspace context">
      <div className="workspace-sidebar-scroll">
        <header className="workspace-sidebar-heading">
          <p className="eyebrow">Current surface</p>
          <h2>{activeTabInfo.title}</h2>
          <p>{activeTabInfo.subtitle}</p>
        </header>

        <section className="workspace-sidebar-section">
          <span className="workspace-sidebar-label">Project</span>
          <ProjectSwitcher
            projectName={projectName}
            onConnectProject={() => onNavigate("projects", "Open the project registry.")}
          />
        </section>

        <section className="workspace-sidebar-section">
          <span className="workspace-sidebar-label">Current engineering state</span>
          <button
            type="button"
            className="workspace-context-row"
            onClick={() => onNavigate("work", hasTask ? "Opened the active Work Item." : "Create or select a Work Item.")}
          >
            <span className={`workspace-context-dot${hasTask ? " ok" : ""}`} />
            <span>
              <strong>{hasTask ? taskTitle : "No active Work Item"}</strong>
              <small>{hasTask ? "Goal, scope and next safe action" : "Work starts from a bounded task"}</small>
            </span>
          </button>
          <button
            type="button"
            className="workspace-context-row"
            onClick={() => onNavigate("changes", dirty ? "Review the current ChangeSet." : "Working tree is clean.")}
          >
            <span className={`workspace-context-dot${!dirty ? " ok" : " warning"}`} />
            <span>
              <strong>{dirty ? `${dirtyCount} workspace changes` : "Working tree clean"}</strong>
              <small>{dirty ? "Review → verify → commit" : "No uncommitted delta"}</small>
            </span>
          </button>
          {!hasProject ? <p className="workspace-sidebar-empty">Connect a project to activate repository-aware surfaces.</p> : null}
        </section>

        <section className="workspace-sidebar-section">
          <span className="workspace-sidebar-label">Navigation</span>
          <p className="workspace-sidebar-empty">
            Use the activity rail for Work, Code, Changes, Runs and Projects. This drawer only describes the current context.
          </p>
        </section>
      </div>

      <footer className="workspace-sidebar-footer">
        <ThemeMenu theme={theme} onChange={onThemeChange} />
        <button type="button" className="workspace-settings-link" onClick={() => onNavigate("settings", "Opened global RepoDesk settings.")}>Settings</button>
      </footer>
    </aside>
  );
}
