import type { TabId, Theme } from "../shared/types/api";
import type { EconomyMode } from "../features/routing/EconomyControl";

export const STORAGE_KEYS = {
  activeTab: "repodesk.activeTab",
  economyMode: "repodesk.economyMode",
  theme: "repodesk.theme",
} as const;

export const DEFAULT_ACTIVE_TAB: TabId = "work";
export const DEFAULT_ECONOMY_MODE: EconomyMode = "balanced";
export const DEFAULT_THEME: Theme = "system";

export const TAB_IDS: readonly TabId[] = [
  "work",
  "changes",
  "history",
  "dashboard",
  "tokens",
  "models",
  "code",
  "git",
  "memory",
  "orchestrate",
  "outcomes",
  "settings",
  "system",
  "debug",
];

export const ECONOMY_MODES: readonly EconomyMode[] = ["economy", "balanced", "quality"] as const;
export const THEMES: readonly Theme[] = [
  "system",
  "dark",
  "light",
  "midnight",
  "nord",
  "high-contrast",
] as const;

/** Theme options with display labels for the theme menu. */
export const THEME_OPTIONS: ReadonlyArray<{ value: Theme; label: string }> = [
  { value: "system", label: "Auto" },
  { value: "dark", label: "Dark" },
  { value: "light", label: "Light" },
  { value: "midnight", label: "Midnight" },
  { value: "nord", label: "Nord" },
  { value: "high-contrast", label: "High contrast" },
];
