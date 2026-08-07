import type { ReactNode } from "react";
import type { TabId } from "../shared/types/api";

// Distinct stroke icons (lucide-style) keep the activity rail scannable without
// adding another icon dependency to the desktop bundle.
function Svg({ children }: { children: ReactNode }) {
  return (
    <svg
      width="20"
      height="20"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {children}
    </svg>
  );
}

export function BurgerIcon() {
  return (
    <Svg>
      <line x1="4" y1="7" x2="20" y2="7" />
      <line x1="4" y1="12" x2="20" y2="12" />
      <line x1="4" y1="17" x2="20" y2="17" />
    </Svg>
  );
}

export function CommandIcon() {
  return (
    <Svg>
      <path d="M9 7V5a2 2 0 1 0-2 2h10a2 2 0 1 0-2-2v14a2 2 0 1 0 2-2H7a2 2 0 1 0 2 2z" />
    </Svg>
  );
}

export function InspectorIcon() {
  return (
    <Svg>
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <path d="M15 4v16" />
      <path d="M17.5 8h1" />
      <path d="M17.5 12h1" />
    </Svg>
  );
}

export function PanelBottomIcon() {
  return (
    <Svg>
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <path d="M3 14h18" />
      <path d="m9 17 2-1.5L9 14" />
    </Svg>
  );
}

export function ChevronIcon({ open }: { open: boolean }) {
  return (
    <Svg>
      {open ? <path d="m7 10 5 5 5-5" /> : <path d="m10 7 5 5-5 5" />}
    </Svg>
  );
}

export function CheckIcon() {
  return (
    <Svg>
      <path d="m5 12 4 4 10-10" />
    </Svg>
  );
}

export function FolderIcon() {
  return (
    <Svg>
      <path d="M3 7.5A2.5 2.5 0 0 1 5.5 5H10l2 2h6.5A2.5 2.5 0 0 1 21 9.5v7A2.5 2.5 0 0 1 18.5 19h-13A2.5 2.5 0 0 1 3 16.5z" />
    </Svg>
  );
}

export function PlusIcon() {
  return (
    <Svg>
      <path d="M12 5v14" />
      <path d="M5 12h14" />
    </Svg>
  );
}

export function CloseIcon() {
  return (
    <Svg>
      <path d="M6 6l12 12" />
      <path d="M18 6 6 18" />
    </Svg>
  );
}

export function ExternalLinkIcon() {
  return (
    <Svg>
      <path d="M14 4h6v6" />
      <path d="M20 4 10 14" />
      <path d="M12 6H6a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-6" />
    </Svg>
  );
}

const ICONS: Record<TabId, ReactNode> = {
  work: (
    <>
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <line x1="9" y1="4" x2="9" y2="20" />
    </>
  ),
  code: (
    <>
      <path d="M9 8l-4 4 4 4" />
      <path d="M15 8l4 4-4 4" />
    </>
  ),
  changes: (
    <>
      <path d="M4 9h13l-4-4" />
      <path d="M20 15H7l4 4" />
    </>
  ),
  history: (
    <>
      <path d="M3 12a9 9 0 1 0 3-6.7L3 8" />
      <path d="M3 4v4h4" />
      <path d="M12 8v4l3 2" />
    </>
  ),
  projects: (
    <>
      <path d="M3 7.5A2.5 2.5 0 0 1 5.5 5H10l2 2h6.5A2.5 2.5 0 0 1 21 9.5v7A2.5 2.5 0 0 1 18.5 19h-13A2.5 2.5 0 0 1 3 16.5z" />
      <path d="M8 11h8M8 14h5" />
    </>
  ),
  settings: (
    <>
      <line x1="4" y1="6" x2="20" y2="6" />
      <circle cx="9" cy="6" r="2" />
      <line x1="4" y1="12" x2="20" y2="12" />
      <circle cx="15" cy="12" r="2" />
      <line x1="4" y1="18" x2="20" y2="18" />
      <circle cx="9" cy="18" r="2" />
    </>
  ),
  "models-cost": (
    <>
      <rect x="3" y="6" width="10" height="10" rx="2" />
      <rect x="6" y="9" width="4" height="4" rx="0.5" />
      <circle cx="17" cy="15" r="4" />
      <path d="M17 13.2v3.6M15.7 14h2.1a.9.9 0 0 1 0 1.8h-1.6a.9.9 0 0 0 0 1.8h2.1" />
    </>
  ),
  dashboard: (
    <>
      <rect x="3" y="3" width="7" height="7" rx="1" />
      <rect x="14" y="3" width="7" height="7" rx="1" />
      <rect x="3" y="14" width="7" height="7" rx="1" />
      <rect x="14" y="14" width="7" height="7" rx="1" />
    </>
  ),
  git: (
    <>
      <circle cx="6" cy="6" r="2.5" />
      <circle cx="6" cy="18" r="2.5" />
      <circle cx="18" cy="8" r="2.5" />
      <path d="M6 8.5v7" />
      <path d="M18 10.5c0 4-6 1.5-6 5.5" />
    </>
  ),
  orchestrate: (
    <>
      <circle cx="6" cy="12" r="2.5" />
      <circle cx="18" cy="6" r="2.5" />
      <circle cx="18" cy="18" r="2.5" />
      <path d="M8.3 10.9 15.7 7.1" />
      <path d="M8.3 13.1 15.7 16.9" />
    </>
  ),
  memory: (
    <>
      <ellipse cx="12" cy="6" rx="8" ry="3" />
      <path d="M4 6v6c0 1.7 3.6 3 8 3s8-1.3 8-3V6" />
      <path d="M4 12v6c0 1.7 3.6 3 8 3s8-1.3 8-3v-6" />
    </>
  ),
  models: (
    <>
      <rect x="6" y="6" width="12" height="12" rx="2" />
      <rect x="9.5" y="9.5" width="5" height="5" rx="1" />
      <path d="M9 2.5v3M15 2.5v3M9 18.5v3M15 18.5v3M2.5 9h3M2.5 15h3M18.5 9h3M18.5 15h3" />
    </>
  ),
  tokens: (
    <>
      <circle cx="9" cy="9" r="6" />
      <path d="M15.5 5.5a6 6 0 0 1 0 13" />
    </>
  ),
  outcomes: (
    <>
      <circle cx="12" cy="12" r="8" />
      <circle cx="12" cy="12" r="4.5" />
      <circle cx="12" cy="12" r="1" />
    </>
  ),
  playbooks: (
    <>
      <path d="M5 4a2 2 0 0 1 2-2h12v18H7a2 2 0 0 0-2 2z" />
      <path d="M19 16H7a2 2 0 0 0-2 2" />
    </>
  ),
  audit: (
    <>
      <path d="M12 3l8 3v6c0 4.6-3.3 7.8-8 9-4.7-1.2-8-4.4-8-9V6z" />
      <path d="M9 12l2 2 4-4" />
    </>
  ),
  system: (
    <>
      <rect x="3" y="4" width="18" height="7" rx="1.5" />
      <rect x="3" y="13" width="18" height="7" rx="1.5" />
      <line x1="7" y1="7.5" x2="7.01" y2="7.5" />
      <line x1="7" y1="16.5" x2="7.01" y2="16.5" />
    </>
  ),
  debug: (
    <>
      <rect x="8" y="6" width="8" height="13" rx="4" />
      <path d="M8 10.5H4M8 14.5H4M16 10.5h4M16 14.5h4M9 5.5 7 3.5M15 5.5 17 3.5M12 3v3" />
    </>
  ),
};

export function TabIcon({ id }: { id: TabId }) {
  return <Svg>{ICONS[id] ?? <circle cx="12" cy="12" r="8" />}</Svg>;
}
