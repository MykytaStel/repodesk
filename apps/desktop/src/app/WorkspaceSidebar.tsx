import type { TabId, Theme } from "../shared/types/api";
import { ProjectSwitcher } from "./ProjectSwitcher";
import { ThemeMenu } from "./ThemeMenu";
import { APP_TABS, type AppTab } from "./tabs";
import { TabIcon } from "./NavIcons";

// Keep the drawer contextual rather than turning it into a second navigation
// tree. The command palette remains the escape hatch for advanced/legacy tools.
const RELATED: Partial<Record<TabId, TabId[]>> = {
  work: ["memory", "orchestrate"],
  code: ["git", "memory"],
  changes: ["git", "audit"],
  history: ["memory", "outcomes"],
  projects: ["memory", "settings", "models-cost"],
  memory: ["work", "history"],
};

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

function RelatedItem({ tab, active, onSelect }: { tab: AppTab; active: boolean; onSelect: () => void }) {
  return (
    <button type="button" className={`workspace-side-link${active ? " active" : ""}`} onClick={onSelect}>
      <span className="workspace-side-link-icon" aria-hidden="true"><TabIcon id={tab.id} /></span>
      <span>
        <strong>{tab.title}</strong>
        <small>{tab.subtitle}</small>
      </span>
    </button>
  );
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
  const explicit = RELATED[activeTab];
  const relatedTabs = (explicit
    ? explicit.map((id) => APP_TABS.find((tab) => tab.id === id)).filter(Boolean)
    : APP_TABS.filter((tab) => tab.group === activeTabInfo.group && tab.id !== activeTab && tab.tier === "more").slice(0, 3)) as AppTab[];

  return (
    <aside className="workspace-sidebar" aria-label="Workspace context">
      <div className="workspace-sidebar-scroll">
        <header className="workspace-sidebar-heading">
          <p className="eyebrow">{activeTabInfo.group}</p>
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
          <span className="workspace-sidebar-label">Current work</span>
          <button
            type="button"
            className="workspace-context-row"
            onClick={() => onNavigate("work", hasTask ? "Opened the active Work Item." : "Create or select a Work Item.")}
          >
            <span className={`workspace-context-dot${hasTask ? " ok" : ""}`} />
            <span>
              <strong>{hasTask ? taskTitle : "No active Work Item"}</strong>
              <small>{hasTask ? "Bounded engineering task" : "Work starts from a bounded task"}</small>
            </span>
          </button>
          <button
            type="button"
            className="workspace-context-row"
            onClick={() => onNavigate("changes", dirty ? "Review workspace changes." : "Working tree is clean.")}
          >
            <span className={`workspace-context-dot${!dirty ? " ok" : " warning"}`} />
            <span>
              <strong>{dirty ? `${dirtyCount} workspace changes` : "Git clean"}</strong>
              <small>{dirty ? "Review before verification" : "No uncommitted changes"}</small>
            </span>
          </button>
          {!hasProject ? <p className="workspace-sidebar-empty">Connect a project to activate repository-aware surfaces.</p> : null}
        </section>

        {relatedTabs.length > 0 ? (
          <section className="workspace-sidebar-section">
            <span className="workspace-sidebar-label">Related</span>
            <div className="workspace-side-links">
              {relatedTabs.map((tab) => (
                <RelatedItem key={tab.id} tab={tab} active={activeTab === tab.id} onSelect={() => onNavigate(tab.id)} />
              ))}
            </div>
          </section>
        ) : null}
      </div>

      <footer className="workspace-sidebar-footer">
        <ThemeMenu theme={theme} onChange={onThemeChange} />
        <button type="button" className="workspace-settings-link" onClick={() => onNavigate("settings", "Opened RepoDesk settings.")}>Settings</button>
      </footer>
    </aside>
  );
}
