import type { ReactNode } from "react";

export function InspectorSection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="semantic-inspector-section">
      <h3>{title}</h3>
      <div className="semantic-inspector-section__body">{children}</div>
    </section>
  );
}
