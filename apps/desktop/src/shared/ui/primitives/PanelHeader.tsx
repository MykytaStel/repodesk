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
  headingLevel = 2,
}: PanelHeaderProps) {
  const Heading = headingLevel === 2 ? "h2" : headingLevel === 3 ? "h3" : "h4";

  return (
    <header className="semantic-panel-header">
      <div className="semantic-panel-header__copy">
        {eyebrow ? <p className="eyebrow">{eyebrow}</p> : null}
        <Heading className="semantic-panel-header__title">{title}</Heading>
        {description ? <div className="semantic-panel-header__description">{description}</div> : null}
      </div>
      {trailing ? <div className="semantic-panel-header__trailing">{trailing}</div> : null}
    </header>
  );
}
