import { CodeTab } from "../features/code/CodeTab";
import { DashboardTab } from "../features/dashboard/DashboardTab";
import { DebugTab } from "../features/debug/DebugTab";
import { GitTab } from "../features/git/GitTab";
import { MemoryTab } from "../features/memory/MemoryTab";
import { ModelsTab } from "../features/models/ModelsTab";
import { EconomyMode } from "../features/routing/EconomyControl";
import { SettingsTab } from "../features/settings/SettingsTab";
import { SystemTab } from "../features/system/SystemTab";
import { TokensTab } from "../features/tokens/TokensTab";
import { WorkflowTab } from "../features/workflow/WorkflowTab";
import type { TabId } from "../shared/types/api";

export const APP_TABS: Array<{ id: TabId; title: string; subtitle: string }> = [
  { id: "dashboard", title: "Dashboard", subtitle: "Daily state" },
  { id: "workflow", title: "Workflow", subtitle: "Next step" },
  { id: "tokens", title: "Tokens", subtitle: "Usage + cost" },
  { id: "models", title: "Models", subtitle: "Runtime health" },
  { id: "code", title: "Code", subtitle: "Changed files" },
  { id: "git", title: "Git", subtitle: "Workspace" },
  { id: "memory", title: "Memory", subtitle: "Project context" },
  { id: "settings", title: "Settings", subtitle: "Providers" },
  { id: "system", title: "System Registry", subtitle: "Skills & MCP" },
  { id: "debug", title: "Debug", subtitle: "Traces" },
];

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
