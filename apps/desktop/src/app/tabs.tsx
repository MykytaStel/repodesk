import { lazy } from "react";
import { EconomyMode } from "../features/routing/EconomyControl";
import type { TabId } from "../shared/types/api";

// Tabs are code-split: only the active tab's chunk is fetched, keeping the
// initial bundle small. Each feature module exports a named component, so the
// lazy loader maps it to a default export.
const DashboardTab = lazy(() => import("../features/dashboard/DashboardTab").then((m) => ({ default: m.DashboardTab })));
const WorkflowTab = lazy(() => import("../features/workflow/WorkflowTab").then((m) => ({ default: m.WorkflowTab })));
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

export type TabGroup = "Work" | "AI" | "System";
export type TabTier = "primary" | "more";

export interface AppTab {
  id: TabId;
  title: string;
  subtitle: string;
  group: TabGroup;
  tier: TabTier;
}

// Primary tabs are the daily spine (kept short so the app leads the user);
// `more` tabs are diagnostics/depth, tucked into a collapsible section.
// Ordered primary-first so ⌘1..9 jump to the everyday surfaces.
export const APP_TABS: AppTab[] = [
  { id: "workflow", title: "Workflow", subtitle: "Home · your 8 steps", group: "Work", tier: "primary" },
  { id: "git", title: "Git", subtitle: "Workspace & diffs", group: "Work", tier: "primary" },
  { id: "code", title: "Code", subtitle: "Changed files + review", group: "Work", tier: "primary" },
  { id: "memory", title: "Memory", subtitle: "Context agents remember", group: "AI", tier: "primary" },
  { id: "orchestrate", title: "Orchestrate", subtitle: "Delegate to sub-agents", group: "AI", tier: "primary" },
  { id: "settings", title: "Settings", subtitle: "Providers & keys", group: "System", tier: "primary" },
  // More — depth & diagnostics, collapsed by default.
  { id: "dashboard", title: "Dashboard", subtitle: "At-a-glance state", group: "Work", tier: "more" },
  { id: "models", title: "Models", subtitle: "Runtime health", group: "AI", tier: "more" },
  { id: "tokens", title: "Tokens", subtitle: "Usage + cost", group: "AI", tier: "more" },
  { id: "outcomes", title: "Outcomes", subtitle: "What it learned", group: "AI", tier: "more" },
  { id: "playbooks", title: "Playbooks", subtitle: "Team recipes", group: "AI", tier: "more" },
  { id: "audit", title: "Audit", subtitle: "Enterprise trail", group: "System", tier: "more" },
  { id: "system", title: "System Registry", subtitle: "Skills & MCP", group: "System", tier: "more" },
  { id: "debug", title: "Debug", subtitle: "Traces", group: "System", tier: "more" },
];

export const PRIMARY_TABS = APP_TABS.filter((tab) => tab.tier === "primary");
export const MORE_TABS = APP_TABS.filter((tab) => tab.tier === "more");

export const TAB_GROUP_ORDER: TabGroup[] = ["Work", "AI", "System"];

export function renderAppTab({
  activeTab,
  economyMode,
  setActiveTab,
  setEconomyMode,
}: {
  activeTab: TabId;
  economyMode: EconomyMode;
  setActiveTab: (tab: TabId) => void;
  setEconomyMode: (mode: EconomyMode) => void;
}) {
  switch (activeTab) {
    case "dashboard":
      return <DashboardTab setActiveTab={setActiveTab} economyMode={economyMode} setEconomyMode={setEconomyMode} />;
    case "workflow":
      return <WorkflowTab economyMode={economyMode} />;
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
      return <PlaybooksTab />;
    default:
      return <DashboardTab setActiveTab={setActiveTab} economyMode={economyMode} setEconomyMode={setEconomyMode} />;
  }
}
