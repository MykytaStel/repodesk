import React, { ReactNode } from "react";
import { ErrorBoundary } from "./ErrorBoundary";

/**
 * Per-tab boundary. Thin wrapper over [`ErrorBoundary`] that scopes reporting to
 * the tab and resets automatically when the active tab changes (so a crashed tab
 * no longer stays broken after you navigate away and back).
 */
export function TabErrorBoundary({ tabId, children }: { tabId: string; children: ReactNode }) {
  return (
    <ErrorBoundary scope={tabId} resetKeys={[tabId]}>
      {children}
    </ErrorBoundary>
  );
}
