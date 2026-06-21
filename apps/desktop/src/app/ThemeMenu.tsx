import { useEffect, useRef, useState } from "react";
import type { Theme } from "../shared/types/api";
import { THEME_OPTIONS } from "./constants";
import { CheckIcon, ChevronIcon } from "./NavIcons";

/**
 * Styled theme picker for the sidebar footer — a custom popover (opens upward)
 * replacing the native <select>, which rendered as an out-of-place OS dropdown.
 */
export function ThemeMenu({ theme, onChange }: { theme: Theme; onChange: (theme: Theme) => void }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    window.addEventListener("mousedown", onClick);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", onClick);
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const current = THEME_OPTIONS.find((opt) => opt.value === theme);

  return (
    <div className="theme-menu" ref={ref}>
      <p className="theme-menu-label">Theme</p>
      <button className="theme-menu-trigger" onClick={() => setOpen((o) => !o)} aria-haspopup="listbox" aria-expanded={open}>
        <span>{current?.label ?? "Theme"}</span>
        <span className="theme-menu-caret" aria-hidden="true">
          <ChevronIcon open />
        </span>
      </button>
      {open && (
        <div className="theme-menu-list" role="listbox">
          {THEME_OPTIONS.map((opt) => (
            <button
              key={opt.value}
              role="option"
              aria-selected={opt.value === theme}
              className={`theme-menu-item ${opt.value === theme ? "active" : ""}`}
              onClick={() => {
                onChange(opt.value as Theme);
                setOpen(false);
              }}
            >
              <span className="theme-menu-check" aria-hidden="true">
                {opt.value === theme ? <CheckIcon /> : null}
              </span>
              <span>{opt.label}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
