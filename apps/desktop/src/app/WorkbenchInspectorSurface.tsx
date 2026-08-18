import { useCallback, useEffect, useRef, type ReactNode } from "react";

interface WorkbenchInspectorSurfaceProps {
  ariaLabel: string;
  eyebrow: string;
  title: ReactNode;
  description?: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  onClose: () => void;
}

export function WorkbenchInspectorSurface({
  ariaLabel,
  eyebrow,
  title,
  description,
  children,
  footer,
  onClose,
}: WorkbenchInspectorSurfaceProps) {
  const openerRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    const activeElement = document.activeElement;
    openerRef.current = activeElement instanceof HTMLElement ? activeElement : null;
  }, []);

  const close = useCallback(() => {
    onClose();
    queueMicrotask(() => {
      const opener = openerRef.current;
      if (opener?.isConnected) opener.focus();
    });
  }, [onClose]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      if (document.querySelector('[role="dialog"][aria-modal="true"]')) return;
      event.preventDefault();
      close();
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [close]);

  return (
    <aside className="workspace-inspector" aria-label={ariaLabel}>
      <div className="workspace-inspector-scroll">
        <header className="workspace-inspector-heading">
          <div className="panel-title-row">
            <p className="eyebrow">{eyebrow}</p>
            <button type="button" className="tiny-button" aria-label="Close inspector" onClick={close}>
              ×
            </button>
          </div>
          <h2>{title}</h2>
          {description ? <p>{description}</p> : null}
        </header>
        {children}
      </div>
      {footer ? <footer className="workspace-inspector-footer">{footer}</footer> : null}
    </aside>
  );
}
