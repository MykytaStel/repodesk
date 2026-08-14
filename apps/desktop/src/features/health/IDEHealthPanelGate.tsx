import { lazy, Suspense } from "react";
import { useRecovery } from "./RecoveryProvider";

const IDEHealthPanel = lazy(() => import("./IDEHealthPanel").then((module) => ({
  default: module.IDEHealthPanel,
})));

export function IDEHealthPanelGate() {
  const { panelOpen } = useRecovery();
  if (!panelOpen) return null;
  return (
    <Suspense fallback={null}>
      <IDEHealthPanel />
    </Suspense>
  );
}
