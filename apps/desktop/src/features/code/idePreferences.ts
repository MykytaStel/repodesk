import { useSyncExternalStore } from "react";

const STORAGE_KEY = "repodesk.ide-preferences.v1";
const EVENT_NAME = "repodesk:ide-preferences";

export type IdePreferences = {
  editorFontSize: number;
  tabSize: 2 | 4 | 8;
  wordWrap: boolean;
  confirmDelete: boolean;
  explorerDensity: "compact" | "comfortable";
};

export const DEFAULT_IDE_PREFERENCES: IdePreferences = {
  editorFontSize: 13,
  tabSize: 2,
  wordWrap: false,
  confirmDelete: true,
  explorerDensity: "compact",
};

let cached = readPreferences();

function clampFontSize(value: unknown): number {
  const parsed = typeof value === "number" ? value : Number(value);
  if (!Number.isFinite(parsed)) return DEFAULT_IDE_PREFERENCES.editorFontSize;
  return Math.min(20, Math.max(10, Math.round(parsed)));
}

function normalizePreferences(input: Partial<IdePreferences> | null | undefined): IdePreferences {
  const tabSize = input?.tabSize === 4 || input?.tabSize === 8 ? input.tabSize : 2;
  return {
    editorFontSize: clampFontSize(input?.editorFontSize),
    tabSize,
    wordWrap: Boolean(input?.wordWrap),
    confirmDelete: input?.confirmDelete !== false,
    explorerDensity: input?.explorerDensity === "comfortable" ? "comfortable" : "compact",
  };
}

function readPreferences(): IdePreferences {
  if (typeof window === "undefined") return DEFAULT_IDE_PREFERENCES;
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    return raw ? normalizePreferences(JSON.parse(raw) as Partial<IdePreferences>) : DEFAULT_IDE_PREFERENCES;
  } catch {
    return DEFAULT_IDE_PREFERENCES;
  }
}

export function getIdePreferences(): IdePreferences {
  return cached;
}

export function saveIdePreferences(update: Partial<IdePreferences>): IdePreferences {
  cached = normalizePreferences({ ...cached, ...update });
  window.localStorage.setItem(STORAGE_KEY, JSON.stringify(cached));
  window.dispatchEvent(new CustomEvent(EVENT_NAME, { detail: cached }));
  return cached;
}

export function resetIdePreferences(): IdePreferences {
  cached = DEFAULT_IDE_PREFERENCES;
  window.localStorage.removeItem(STORAGE_KEY);
  window.dispatchEvent(new CustomEvent(EVENT_NAME, { detail: cached }));
  return cached;
}

function subscribe(listener: () => void): () => void {
  const handler = () => {
    cached = readPreferences();
    listener();
  };
  window.addEventListener(EVENT_NAME, handler);
  window.addEventListener("storage", handler);
  return () => {
    window.removeEventListener(EVENT_NAME, handler);
    window.removeEventListener("storage", handler);
  };
}

export function useIdePreferences(): IdePreferences {
  return useSyncExternalStore(subscribe, getIdePreferences, () => DEFAULT_IDE_PREFERENCES);
}
