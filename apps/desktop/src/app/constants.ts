import type { TabId, Theme } from "../shared/types/api";
import type { EconomyMode } from "../features/routing/EconomyControl";

export const STORAGE_KEYS = {
  activeTab: "repodesk.activeTab",
  economyMode: "repodesk.economyMode",
  theme: "repodesk.theme",
  sidebarCollapsed: "repodesk.sidebarCollapsed",
  inspectorOpen: "repodesk.inspectorOpen",
  bottomPanelOpen: "repodesk.bottomPanelOpen",
} as const;

export const DEFAULT_ACTIVE_TAB: TabId = "work";
export const DEFAULT_ECONOMY_MODE: EconomyMode = "balanced";
export const DEFAULT_THEME: Theme = "system";

// Keep historical IDs readable so persisted state and old deep links can be
// migrated without keeping duplicate product destinations alive.
export const TAB_IDS: readonly TabId[] = [
  "work",
  "code",
  "changes",
  "history",
  "projects",
  "settings",
  "debug",
  "dashboard",
  "tokens",
  "models",
  "git",
  "memory",
  "orchestrate",
  "outcomes",
  "playbooks",
  "models-cost",
  "audit",
  "system",
];

export const LEGACY_TAB_ALIASES: Readonly<Partial<Record<TabId, TabId>>> = {
  dashboard: "work",
  git: "changes",
  orchestrate: "work",
  outcomes: "history",
  audit: "history",
  memory: "projects",
  playbooks: "projects",
  "models-cost": "settings",
  models: "settings",
  tokens: "settings",
  system: "settings",
};

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
