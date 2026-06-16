import { CodeTab } from "../features/code/CodeTab";
import { DashboardTab } from "../features/dashboard/DashboardTab";
import { DebugTab } from "../features/debug/DebugTab";
import { GitTab } from "../features/git/GitTab";
import { MemoryTab } from "../features/memory/MemoryTab";
import { ModelsTab } from "../features/models/ModelsTab";
import { OrchestrateTab } from "../features/orchestrate/OrchestrateTab";
import { EconomyMode } from "../features/routing/EconomyControl";
import { SettingsTab } from "../features/settings/SettingsTab";
import { SystemTab } from "../features/system/SystemTab";
import { TokensTab } from "../features/tokens/TokensTab";
import { WorkflowTab } from "../features/workflow/WorkflowTab";
import type { TabId } from "../shared/types/api";

export type TabGroup = "Work" | "AI" | "System";

export const APP_TABS: Array<{ id: TabId; title: string; subtitle: string; group: TabGroup }> = [
  // Work — the daily loop; Workflow is the home surface.
  { id: "workflow", title: "Workflow", subtitle: "Next step", group: "Work" },
  { id: "dashboard", title: "Dashboard", subtitle: "Daily state", group: "Work" },
  { id: "git", title: "Git", subtitle: "Workspace", group: "Work" },
  { id: "code", title: "Code", subtitle: "Changed files", group: "Work" },
  // AI — providers, routing, memory, orchestration.
  { id: "models", title: "Models", subtitle: "Runtime health", group: "AI" },
  { id: "tokens", title: "Tokens", subtitle: "Usage + cost", group: "AI" },
  { id: "memory", title: "Memory", subtitle: "Project context", group: "AI" },
  { id: "orchestrate", title: "Orchestrate", subtitle: "Sub-agents", group: "AI" },
  // System — configuration and diagnostics.
  { id: "settings", title: "Settings", subtitle: "Providers", group: "System" },
  { id: "system", title: "System Registry", subtitle: "Skills & MCP", group: "System" },
  { id: "debug", title: "Debug", subtitle: "Traces", group: "System" },
];

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
    case "settings":
      return <SettingsTab />;
    case "system":
      return <SystemTab />;
    case "debug":
      return <DebugTab />;
    default:
      return <DashboardTab setActiveTab={setActiveTab} economyMode={economyMode} setEconomyMode={setEconomyMode} />;
  }
}
