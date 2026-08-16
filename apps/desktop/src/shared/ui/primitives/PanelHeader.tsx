import type { ReactNode } from "react";

export type PanelHeaderProps = {
  eyebrow?: string;
  title: ReactNode;
  description?: ReactNode;
  trailing?: ReactNode;
  headingLevel?: 2 | 3 | 4;
};

export function PanelHeader({
  eyebrow,
  title,
  description,
  trailing,
  headingLevel,
}: PanelHeaderProps) {
  const Heading = headingLevel ? (`h${headingLevel}` as const) : null;

  return (
    <header className="semantic-panel-header">
      <div className="semantic-panel-header__copy">
        {eyebrow ? <p className="eyebrow">{eyebrow}</p> : null}
        {Heading ? (
          <Heading className="semantic-panel-header__title">{title}</Heading>
        ) : (
          <div className="semantic-panel-header__title">{title}</div>
        )}
        {description ? <div className="semantic-panel-header__description">{description}</div> : null}
      </div>
      {trailing ? <div className="semantic-panel-header__trailing">{trailing}</div> : null}
    </header>
  );
}
