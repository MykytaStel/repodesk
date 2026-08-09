export type BottomPanelTab = "problems" | "tasks" | "output" | "terminal";

export const BOTTOM_PANEL_TAB_EVENT = "repodesk:bottom-panel-tab";

/**
 * Request one secondary bottom-dock surface without coupling the shell to the
 * WorkbenchBottomPanel's internal React state. The panel remains the owner of
 * tab lifetime; callers only express navigation intent.
 */
export function requestBottomPanelTab(tab: BottomPanelTab): void {
  window.dispatchEvent(new CustomEvent<BottomPanelTab>(BOTTOM_PANEL_TAB_EVENT, { detail: tab }));
}
