import type { TabId } from "../../shared/types/api";
import { WorkItemContractCard } from "./WorkItemContractCard";
import { WorkTab } from "./WorkTab";

export function WorkSurface({ setActiveTab }: { setActiveTab: (tab: TabId) => void }) {
  return (
    <div className="work-surface-v2">
      <WorkItemContractCard />
      <WorkTab setActiveTab={setActiveTab} />
    </div>
  );
}
