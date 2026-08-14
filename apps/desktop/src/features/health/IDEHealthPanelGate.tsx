import { lazy, Suspense } from "react";
import { ErrorBoundary } from "../../shared/ui/ErrorBoundary";
import { useRecovery } from "./RecoveryProvider";

const IDEHealthPanel = lazy(() => import("./IDEHealthPanel").then((module) => ({
  default: module.IDEHealthPanel,
})));

export function IDEHealthPanelGate() {
  const { panelOpen } = useRecovery();
  if (!panelOpen) return null;
  return (
    <ErrorBoundary scope="ide-health-panel" resetKeys={[panelOpen]}>
      <Suspense fallback={null}>
        <IDEHealthPanel />
      </Suspense>
    </ErrorBoundary>
  );
}
