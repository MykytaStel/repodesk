import type { ReactNode } from "react";

export type PanelHeaderProps = {
  eyebrow?: string;
  title: ReactNode;
  description?: ReactNode;
  trailing?: ReactNode;
};

export function PanelHeader({ eyebrow, title, description, trailing }: PanelHeaderProps) {
  return (
    <header className="semantic-panel-header">
      <div className="semantic-panel-header__copy">
        {eyebrow ? <p className="eyebrow">{eyebrow}</p> : null}
        <div className="semantic-panel-header__title">{title}</div>
        {description ? <div className="semantic-panel-header__description">{description}</div> : null}
      </div>
      {trailing ? <div className="semantic-panel-header__trailing">{trailing}</div> : null}
    </header>
  );
}
