import { open } from "@tauri-apps/plugin-dialog";

/**
 * Open a native folder picker and return the chosen absolute path, or `null`
 * if the user cancelled. Used to select a project directory instead of typing
 * the path by hand.
 */
export async function pickDirectory(title = "Choose project folder"): Promise<string | null> {
  const selected = await open({ directory: true, multiple: false, title });
  // With `multiple: false` the plugin returns a single path string (or null).
  return typeof selected === "string" ? selected : null;
}

/** The trailing path segment of an absolute path — a sensible default project name. */
export function basename(path: string): string {
  const parts = path.split(/[/\\]/).filter(Boolean);
  return parts.length ? parts[parts.length - 1] : path;
}
