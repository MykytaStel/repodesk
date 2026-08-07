import { lazy } from "react";
import { EconomyMode } from "../features/routing/EconomyControl";
import type { TabId } from "../shared/types/api";

// Tabs remain code-split. RD2-07 changes the shell hierarchy, not feature ownership:
// Work/Code/Changes/Runs/Projects become primary IDE surfaces while deeper tools
// stay routable from the contextual sidebar and command palette.
const WorkSurface = lazy(() => import("../features/work/WorkSurface").then((m) => ({ default: m.WorkSurface })));
const ChangesTab = lazy(() => import("../features/changes/ChangesTab").then((m) => ({ default: m.ChangesTab })));
const HistoryTab = lazy(() => import("../features/history/HistoryTab").then((m) => ({ default: m.HistoryTab })));
const ProjectsTab = lazy(() => import("../features/projects/ProjectsTab").then((m) => ({ default: m.ProjectsTab })));
const DashboardTab = lazy(() => import("../features/dashboard/DashboardTab").then((m) => ({ default: m.DashboardTab })));
const TokensTab = lazy(() => import("../features/tokens/TokensTab").then((m) => ({ default: m.TokensTab })));
const ModelsTab = lazy(() => import("../features/models/ModelsTab").then((m) => ({ default: m.ModelsTab })));
const CodeTab = lazy(() => import("../features/code/CodeTab").then((m) => ({ default: m.CodeTab })));
const GitTab = lazy(() => import("../features/git/GitTab").then((m) => ({ default: m.GitTab })));
const MemoryTab = lazy(() => import("../features/memory/MemoryTab").then((m) => ({ default: m.MemoryTab })));
const OrchestrateTab = lazy(() => import("../features/orchestrate/OrchestrateTab").then((m) => ({ default: m.OrchestrateTab })));
const OutcomesTab = lazy(() => import("../features/outcomes/OutcomesTab").then((m) => ({ default: m.OutcomesTab })));
const SettingsTab = lazy(() => import("../features/settings/SettingsTab").then((m) => ({ default: m.SettingsTab })));
const SystemTab = lazy(() => import("../features/system/SystemTab").then((m) => ({ default: m.SystemTab })));
const DebugTab = lazy(() => import("../features/debug/DebugTab").then((m) => ({ default: m.DebugTab })));
const AuditTab = lazy(() => import("../features/audit/AuditTab").then((m) => ({ default: m.AuditTab })));
const PlaybooksTab = lazy(() => import("../features/playbooks/PlaybooksTab").then((m) => ({ default: m.PlaybooksTab })));
const ModelsCostTab = lazy(() => import("../features/models-cost/ModelsCostTab").then((m) => ({ default: m.ModelsCostTab })));

export type TabGroup = "Work" | "AI" | "System";
export type TabTier = "primary" | "more" | "hidden";

export interface AppTab {
  id: TabId;
  title: string;
  subtitle: string;
  group: TabGroup;
  tier: TabTier;
}

export const APP_TABS: AppTab[] = [
  // IDE activity rail — stable primary engineering surfaces.
  { id: "work", title: "Work", subtitle: "Work Item · Scope → Finish", group: "Work", tier: "primary" },
  { id: "code", title: "Code", subtitle: "Files, findings & analysis", group: "Work", tier: "primary" },
  { id: "changes", title: "Changes", subtitle: "Diffs, review & git delta", group: "Work", tier: "primary" },
  { id: "history", title: "Runs", subtitle: "Executions, evidence & history", group: "AI", tier: "primary" },
  { id: "projects", title: "Projects", subtitle: "Repository workspaces", group: "System", tier: "primary" },

  // Contextual tools — available beside the active workspace surface.
  { id: "dashboard", title: "Dashboard", subtitle: "At-a-glance state", group: "Work", tier: "more" },
  { id: "git", title: "Git", subtitle: "Workspace & diffs", group: "Work", tier: "more" },
  { id: "orchestrate", title: "Orchestrate", subtitle: "Delegate to workers", group: "AI", tier: "more" },
  { id: "playbooks", title: "Playbooks", subtitle: "Engineering recipes", group: "AI", tier: "more" },
  { id: "models-cost", title: "Models & Cost", subtitle: "Runtime health + spend", group: "AI", tier: "more" },
  { id: "settings", title: "Settings", subtitle: "Providers, keys & policy", group: "System", tier: "more" },
  { id: "system", title: "System Registry", subtitle: "Skills & MCP", group: "System", tier: "more" },
  { id: "debug", title: "Debug", subtitle: "Traces", group: "System", tier: "more" },

  // Deep surfaces remain routable for command palette/deep-links.
  { id: "memory", title: "Memory", subtitle: "Engineering knowledge", group: "AI", tier: "hidden" },
  { id: "models", title: "Models", subtitle: "Runtime health", group: "AI", tier: "hidden" },
  { id: "tokens", title: "Tokens", subtitle: "Usage + cost", group: "AI", tier: "hidden" },
  { id: "outcomes", title: "Outcomes", subtitle: "What executions learned", group: "AI", tier: "hidden" },
  { id: "audit", title: "Audit", subtitle: "Engineering trail", group: "System", tier: "hidden" },
];

export const PRIMARY_TABS = APP_TABS.filter((tab) => tab.tier === "primary");
export const MORE_TABS = APP_TABS.filter((tab) => tab.tier === "more");
export const NAV_TABS = APP_TABS.filter((tab) => tab.tier !== "hidden");
export const TAB_GROUP_ORDER: TabGroup[] = ["Work", "AI", "System"];

export function renderAppTab({
  activeTab,
  economyMode,
  setActiveTab,
  setEconomyMode,
}: {
  activeTab: TabId;
  economyMode: EconomyMode;
  setActiveTab: (tab: TabId, detail?: string) => void;
  setEconomyMode: (mode: EconomyMode) => void;
}) {
  switch (activeTab) {
    case "work":
      return <WorkSurface setActiveTab={setActiveTab} />;
    case "code":
      return <CodeTab />;
    case "changes":
      return <ChangesTab />;
    case "history":
      return <HistoryTab />;
    case "projects":
      return <ProjectsTab setActiveTab={setActiveTab} />;
    case "models-cost":
      return <ModelsCostTab setActiveTab={setActiveTab} />;
    case "dashboard":
      return <DashboardTab setActiveTab={setActiveTab} economyMode={economyMode} setEconomyMode={setEconomyMode} />;
    case "tokens":
      return <TokensTab />;
    case "models":
      return <ModelsTab setActiveTab={setActiveTab} />;
    case "git":
      return <GitTab />;
    case "memory":
      return <MemoryTab />;
    case "orchestrate":
      return <OrchestrateTab setActiveTab={setActiveTab} />;
    case "outcomes":
      return <OutcomesTab />;
    case "settings":
      return <SettingsTab />;
    case "system":
      return <SystemTab />;
    case "debug":
      return <DebugTab />;
    case "audit":
      return <AuditTab />;
    case "playbooks":
      return <PlaybooksTab setActiveTab={setActiveTab} />;
    default:
      return <WorkSurface setActiveTab={setActiveTab} />;
  }
}
