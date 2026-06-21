import { lazy } from "react";
import { EconomyMode } from "../features/routing/EconomyControl";
import type { TabId } from "../shared/types/api";

// Tabs are code-split: only the active tab's chunk is fetched, keeping the
// initial bundle small. Each feature module exports a named component, so the
// lazy loader maps it to a default export.
const WorkTab = lazy(() => import("../features/work/WorkTab").then((m) => ({ default: m.WorkTab })));
const ChangesTab = lazy(() => import("../features/changes/ChangesTab").then((m) => ({ default: m.ChangesTab })));
const HistoryTab = lazy(() => import("../features/history/HistoryTab").then((m) => ({ default: m.HistoryTab })));
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
// `primary` — the daily spine in the sidebar; `more` — depth/diagnostics in the
// collapsible groups; `hidden` — surfaces fully absorbed into a primary tab
// (Git/Code→Changes, Outcomes/Memory/Audit→History, Tokens/Models→Models & Cost).
// Hidden tabs stay routable for deep-links and ⌘K but never render in the rail.
export type TabTier = "primary" | "more" | "hidden";

export interface AppTab {
  id: TabId;
  title: string;
  subtitle: string;
  group: TabGroup;
  tier: TabTier;
}

// Primary tabs are the daily spine (kept short so the app leads the user);
// `more` tabs are diagnostics/depth, tucked into collapsible group sections.
// Ordered primary-first so ⌘1..9 jump to the everyday surfaces. Each tab's icon
// is rendered by `TabIcon` (NavIcons.tsx), keyed by id.
export const APP_TABS: AppTab[] = [
  // Primary spine — the five surfaces that carry the daily flow.
  { id: "work", title: "Work", subtitle: "Home · Scope → Finish", group: "Work", tier: "primary" },
  { id: "changes", title: "Changes", subtitle: "Diffs, files & review", group: "Work", tier: "primary" },
  { id: "history", title: "History", subtitle: "Runs, memory & audit", group: "AI", tier: "primary" },
  { id: "models-cost", title: "Models & Cost", subtitle: "Reachable models + spend", group: "AI", tier: "primary" },
  { id: "settings", title: "Settings", subtitle: "Providers & keys", group: "System", tier: "primary" },
  // Advanced — depth & diagnostics, grouped and collapsible.
  { id: "dashboard", title: "Dashboard", subtitle: "At-a-glance state", group: "Work", tier: "more" },
  { id: "orchestrate", title: "Orchestrate", subtitle: "Delegate to sub-agents", group: "AI", tier: "more" },
  { id: "playbooks", title: "Playbooks", subtitle: "Team recipes", group: "AI", tier: "more" },
  { id: "system", title: "System Registry", subtitle: "Skills & MCP", group: "System", tier: "more" },
  { id: "debug", title: "Debug", subtitle: "Traces", group: "System", tier: "more" },
  // Hidden — absorbed into a primary surface; reachable via ⌘K + deep-links only.
  { id: "git", title: "Git", subtitle: "Workspace & diffs", group: "Work", tier: "hidden" },
  { id: "code", title: "Code", subtitle: "Changed files + review", group: "Work", tier: "hidden" },
  { id: "memory", title: "Memory", subtitle: "Context agents remember", group: "AI", tier: "hidden" },
  { id: "models", title: "Models", subtitle: "Runtime health", group: "AI", tier: "hidden" },
  { id: "tokens", title: "Tokens", subtitle: "Usage + cost", group: "AI", tier: "hidden" },
  { id: "outcomes", title: "Outcomes", subtitle: "What it learned", group: "AI", tier: "hidden" },
  { id: "audit", title: "Audit", subtitle: "Enterprise trail", group: "System", tier: "hidden" },
];

export const PRIMARY_TABS = APP_TABS.filter((tab) => tab.tier === "primary");
export const MORE_TABS = APP_TABS.filter((tab) => tab.tier === "more");
// Everything the sidebar offers (rail + groups); excludes hidden/absorbed tabs.
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
      return <WorkTab setActiveTab={setActiveTab} />;
    case "changes":
      return <ChangesTab />;
    case "history":
      return <HistoryTab />;
    case "models-cost":
      return <ModelsCostTab setActiveTab={setActiveTab} />;
    case "dashboard":
      return <DashboardTab setActiveTab={setActiveTab} economyMode={economyMode} setEconomyMode={setEconomyMode} />;
    case "tokens":
      return <TokensTab />;
    case "models":
      return <ModelsTab setActiveTab={setActiveTab} />;
    case "code":
      return <CodeTab />;
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
      return <DashboardTab setActiveTab={setActiveTab} economyMode={economyMode} setEconomyMode={setEconomyMode} />;
  }
}
