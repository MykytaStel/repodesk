import type { TabId } from "../../shared/types/api";
import { ContextInspectorCard } from "./ContextInspectorCard";
import { EngineeringIntelligenceCard } from "./EngineeringIntelligenceCard";
import { WorkTab } from "./WorkTab";

export function WorkSurface({ setActiveTab }: { setActiveTab: (tab: TabId) => void }) {
  return (
    <>
      <WorkTab setActiveTab={setActiveTab} />
      <ContextInspectorCard />
      <EngineeringIntelligenceCard />
    </>
  );
}
