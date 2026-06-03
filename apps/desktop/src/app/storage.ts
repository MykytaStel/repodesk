import type { TabId, Theme } from "../shared/types/api";
import type { EconomyMode } from "../features/routing/EconomyControl";
import {
  DEFAULT_ACTIVE_TAB,
  DEFAULT_ECONOMY_MODE,
  DEFAULT_THEME,
  ECONOMY_MODES,
  STORAGE_KEYS,
  TAB_IDS,
  THEMES,
} from "./constants";

export function readStoredActiveTab(): TabId {
  return readStoredValue(STORAGE_KEYS.activeTab, TAB_IDS, DEFAULT_ACTIVE_TAB);
}

export function readStoredEconomyMode(): EconomyMode {
  return readStoredValue(STORAGE_KEYS.economyMode, ECONOMY_MODES, DEFAULT_ECONOMY_MODE);
}

export function readStoredTheme(): Theme {
  return readStoredValue(STORAGE_KEYS.theme, THEMES, DEFAULT_THEME);
}

function readStoredValue<T extends string>(key: string, allowed: readonly T[], fallback: T): T {
  const value = window.localStorage.getItem(key);
  return allowed.includes(value as T) ? (value as T) : fallback;
}
