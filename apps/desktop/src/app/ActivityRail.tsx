import { useEffect, type ReactNode } from "react";
import { CODE_OPEN_EVENT } from "../shared/api/codeWorkspace";
import type { TabId } from "../shared/types/api";
import type { AppTab } from "./tabs";
import {
  BurgerIcon,
  CommandIcon,
  InspectorIcon,
  PanelBottomIcon,
  TabIcon,
} from "./NavIcons";

interface ActivityRailProps {
  activeTab: TabId;
  tabs: AppTab[];
  sidebarOpen: boolean;
  inspectorOpen: boolean;
  bottomPanelOpen: boolean;
  appVersion: string;
  onSelect: (tab: TabId) => void;
  onToggleSidebar: () => void;
  onToggleInspector: () => void;
  onToggleBottomPanel: () => void;
  onOpenPalette: () => void;
}

function RailButton({
  label,
  active = false,
  onClick,
  children,
}: {
  label: string;
  active?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      className={`activity-rail-button${active ? " active" : ""}`}
      onClick={onClick}
      aria-label={label}
      title={label}
      aria-pressed={active}
    >
      {children}
    </button>
  );
}

export function ActivityRail({
  activeTab,
  tabs,
  sidebarOpen,
  inspectorOpen,
  bottomPanelOpen,
  appVersion,
  onSelect,
  onToggleSidebar,
  onToggleInspector,
  onToggleBottomPanel,
  onOpenPalette,
}: ActivityRailProps) {
  // Diagnostics and other secondary surfaces can request an exact Code location
  // without coupling themselves to App's route state. The Code workspace still
  // consumes and validates the one-shot path/location request separately.
  useEffect(() => {
    const openCode = () => onSelect("code");
    window.addEventListener(CODE_OPEN_EVENT, openCode);
    return () => window.removeEventListener(CODE_OPEN_EVENT, openCode);
  }, [onSelect]);

  return (
    <aside className="activity-rail" aria-label="Primary workspace navigation">
      <div className="activity-rail-top">
        <div className="activity-brand" title={`RepoDesk v${appVersion}`} aria-label={`RepoDesk version ${appVersion}`}>
          RD
        </div>

        <RailButton label={sidebarOpen ? "Hide workspace sidebar" : "Show workspace sidebar"} active={sidebarOpen} onClick={onToggleSidebar}>
          <BurgerIcon />
        </RailButton>

        <div className="activity-rail-divider" />

        {tabs.map((tab) => (
          <RailButton
            key={tab.id}
            label={`${tab.title} — ${tab.subtitle}`}
            active={activeTab === tab.id}
            onClick={() => onSelect(tab.id)}
          >
            <TabIcon id={tab.id} />
          </RailButton>
        ))}
      </div>

      <div className="activity-rail-bottom">
        <RailButton label="Command palette" onClick={onOpenPalette}>
          <CommandIcon />
        </RailButton>
        <RailButton label={inspectorOpen ? "Hide inspector" : "Show inspector"} active={inspectorOpen} onClick={onToggleInspector}>
          <InspectorIcon />
        </RailButton>
        <RailButton label={bottomPanelOpen ? "Hide bottom panel" : "Show bottom panel"} active={bottomPanelOpen} onClick={onToggleBottomPanel}>
          <PanelBottomIcon />
        </RailButton>
      </div>
    </aside>
  );
}
