import type { SVGProps } from "react";

export type IdeIconName =
  | "search"
  | "file-add"
  | "folder-add"
  | "refresh"
  | "collapse"
  | "rename"
  | "delete"
  | "copy"
  | "context"
  | "analyze"
  | "changes"
  | "more";

export function IdeIcon({ name, ...props }: { name: IdeIconName } & SVGProps<SVGSVGElement>) {
  return (
    <svg
      viewBox="0 0 16 16"
      width="16"
      height="16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.35"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
      {...props}
    >
      {name === "search" ? <><circle cx="6.8" cy="6.8" r="4.1" /><path d="m10 10 3.2 3.2" /></> : null}
      {name === "file-add" ? <><path d="M3.5 1.8h5l3 3v9.4h-8z" /><path d="M8.5 1.8v3h3M7.5 7v4M5.5 9h4" /></> : null}
      {name === "folder-add" ? <><path d="M1.8 4.2h4l1.2 1.4h7.2v7.5H1.8z" /><path d="M8 7.3v3.7M6.2 9.2h3.6" /></> : null}
      {name === "refresh" ? <><path d="M12.7 5.3A5.2 5.2 0 1 0 13 10" /><path d="M10.5 2.8h2.7v2.7" /></> : null}
      {name === "collapse" ? <><path d="M4 6.2 8 10l4-3.8" /><path d="M4 2.8 8 6.6l4-3.8" /></> : null}
      {name === "rename" ? <><path d="M3 12.8h3.1l7-7-3.1-3.1-7 7z" /><path d="m8.9 3.8 3.1 3.1M2.5 14h11" /></> : null}
      {name === "delete" ? <><path d="M3.2 4.3h9.6M6 2.2h4l.7 2.1H5.3zM4.6 4.3l.6 9.3h5.6l.6-9.3M6.8 6.6v4.7M9.2 6.6v4.7" /></> : null}
      {name === "copy" ? <><rect x="5.2" y="4.8" width="7.4" height="8" rx="1" /><path d="M3.3 10.7H2.5a1 1 0 0 1-1-1V2.5a1 1 0 0 1 1-1h7.2a1 1 0 0 1 1 1v.7" /></> : null}
      {name === "context" ? <><path d="M2.2 3.2h11.6v9.6H2.2z" /><path d="M5 6h6M5 8.2h4.2M5 10.4h2.8" /></> : null}
      {name === "analyze" ? <><path d="M2.2 13.3 5.5 9l2.4 2.1 5.9-7.2" /><path d="M10.5 3.9h3.3v3.3" /></> : null}
      {name === "changes" ? <><path d="M4 2.4v8.1a2.4 2.4 0 0 0 2.4 2.4H12" /><circle cx="4" cy="2.5" r="1.3" /><circle cx="12" cy="12.9" r="1.3" /><path d="M8.2 4.3h3.8M10.7 2.8 12.2 4.3l-1.5 1.5" /></> : null}
      {name === "more" ? <><circle cx="3.2" cy="8" r=".7" fill="currentColor" stroke="none" /><circle cx="8" cy="8" r=".7" fill="currentColor" stroke="none" /><circle cx="12.8" cy="8" r=".7" fill="currentColor" stroke="none" /></> : null}
    </svg>
  );
}
