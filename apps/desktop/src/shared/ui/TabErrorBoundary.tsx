import React, { Component, ReactNode } from "react";

interface Props {
  tabId: string;
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export class TabErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    console.error(`[RepoDesk] Tab "${this.props.tabId}" crashed:`, error, info.componentStack);
  }

  handleReset = () => {
    this.setState({ hasError: false, error: null });
  };

  render() {
    if (this.state.hasError) {
      return (
        <div className="content-grid">
          <section className="panel wide-panel" style={{ display: "flex", flexDirection: "column", alignItems: "center", padding: "48px 24px", textAlign: "center" }}>
            <p className="eyebrow" style={{ color: "var(--danger)" }}>Error</p>
            <h2 style={{ color: "var(--danger)", marginTop: 8 }}>Tab crashed</h2>
            <p className="muted" style={{ marginTop: 12, maxWidth: 480 }}>
              {this.state.error?.message || "An unexpected error occurred in this tab."}
            </p>
            <div className="button-row" style={{ justifyContent: "center" }}>
              <button className="primary-button" onClick={this.handleReset}>
                Try again
              </button>
            </div>
          </section>
        </div>
      );
    }

    return this.props.children;
  }
}
