import { lazy } from "react";
import type { EconomyMode } from "../features/routing/EconomyControl";
import type { TabId } from "../shared/types/api";
import { LEGACY_TAB_ALIASES } from "./constants";

const WorkSurface = lazy(() => import("../features/work/WorkSurface").then((m) => ({ default: m.WorkSurface })));
const CodeTab = lazy(() => import("../features/code/CodeTab").then((m) => ({ default: m.CodeTab })));
const ChangesTab = lazy(() => import("../features/changes/ChangesTab").then((m) => ({ default: m.ChangesTab })));
const HistoryTab = lazy(() => import("../features/history/HistoryTab").then((m) => ({ default: m.HistoryTab })));
const ProjectsTab = lazy(() => import("../features/projects/ProjectsTab").then((m) => ({ default: m.ProjectsTab })));
const SettingsTab = lazy(() => import("../features/settings/SettingsTab").then((m) => ({ default: m.SettingsTab })));
const DebugTab = lazy(() => import("../features/debug/DebugTab").then((m) => ({ default: m.DebugTab })));

export type TabGroup = "Work" | "AI" | "System";
export type TabTier = "primary" | "more" | "hidden";

export interface AppTab {
  id: TabId;
  title: string;
  subtitle: string;
  group: TabGroup;
  tier: TabTier;
}

// Product navigation has exactly five engineering destinations. Settings and
// Debug are utilities, not parallel interpretations of the same engineering
// state. Historical route IDs are handled only by LEGACY_TAB_ALIASES.
export const APP_TABS: AppTab[] = [
  { id: "work", title: "Work", subtitle: "Work Item · Scope → Finish", group: "Work", tier: "primary" },
  { id: "code", title: "Code", subtitle: "Repository explorer & editor", group: "Work", tier: "primary" },
  { id: "changes", title: "Changes", subtitle: "ChangeSet · review · verification", group: "Work", tier: "primary" },
  { id: "history", title: "Runs", subtitle: "Executions, receipts & evidence", group: "AI", tier: "primary" },
  { id: "projects", title: "Projects", subtitle: "Rules, knowledge & work templates", group: "System", tier: "primary" },
  { id: "settings", title: "Settings", subtitle: "Global providers, keys & preferences", group: "System", tier: "more" },
  { id: "debug", title: "Debug", subtitle: "Developer traces", group: "System", tier: "hidden" },
];

export const PRIMARY_TABS = APP_TABS.filter((tab) => tab.tier === "primary");
export const MORE_TABS = APP_TABS.filter((tab) => tab.tier === "more");
export const NAV_TABS = APP_TABS.filter((tab) => tab.tier !== "hidden");
export const TAB_GROUP_ORDER: TabGroup[] = ["Work", "AI", "System"];

export function canonicalTabId(tabId: TabId): TabId {
  return LEGACY_TAB_ALIASES[tabId] ?? tabId;
}

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
  void economyMode;
  void setEconomyMode;

  switch (canonicalTabId(activeTab)) {
    case "work":
      return <WorkSurface setActiveTab={setActiveTab} />;
    case "code":
      return <CodeTab setActiveTab={setActiveTab} />;
    case "changes":
      return <ChangesTab setActiveTab={setActiveTab} />;
    case "history":
      return <HistoryTab />;
    case "projects":
      return <ProjectsTab setActiveTab={setActiveTab} />;
    case "settings":
      return <SettingsTab />;
    case "debug":
      return <DebugTab />;
    default:
      return <WorkSurface setActiveTab={setActiveTab} />;
  }
}
