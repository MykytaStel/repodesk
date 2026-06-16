import type { TabId, Theme } from "../shared/types/api";
import type { EconomyMode } from "../features/routing/EconomyControl";

export const STORAGE_KEYS = {
  activeTab: "repodesk.activeTab",
  economyMode: "repodesk.economyMode",
  theme: "repodesk.theme",
} as const;

export const DEFAULT_ACTIVE_TAB: TabId = "workflow";
export const DEFAULT_ECONOMY_MODE: EconomyMode = "balanced";
export const DEFAULT_THEME: Theme = "system";

export const TAB_IDS: readonly TabId[] = [
  "dashboard",
  "workflow",
  "tokens",
  "models",
  "code",
  "git",
  "memory",
  "settings",
  "system",
  "debug",
];

export const ECONOMY_MODES: readonly EconomyMode[] = ["economy", "balanced", "quality"] as const;
export const THEMES: readonly Theme[] = ["dark", "light", "system"] as const;
