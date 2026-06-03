import React, { Component, ReactNode } from "react";
import { ErrorState } from "./ErrorState";
import { reportError } from "../utils/errors";

interface Props {
  /** Label used in reporting + the fallback (e.g. "app" or the tab id). */
  scope: string;
  children: ReactNode;
  /** Render a centered, full-screen fallback (use for the root boundary). */
  fullscreen?: boolean;
  /**
   * When any value here changes, a captured error is cleared automatically.
   * Pass `[activeTab]` so navigating away from a crashed tab recovers it.
   */
  resetKeys?: unknown[];
}

interface State {
  error: Error | null;
}

/**
 * Generic React error boundary. Catches render/lifecycle crashes in its subtree,
 * reports them through the central reporter (console + Debug tab + event
 * journal), and renders a recoverable [`ErrorState`] fallback.
 */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    reportError(`boundary:${this.props.scope}`, error, {
      componentStack: (info.componentStack ?? "").trim().slice(0, 600),
    });
  }

  componentDidUpdate(prev: Props) {
    if (this.state.error && !sameKeys(prev.resetKeys, this.props.resetKeys)) {
      this.setState({ error: null });
    }
  }

  reset = () => this.setState({ error: null });

  render() {
    if (this.state.error) {
      return (
        <ErrorState
          scope={this.props.scope}
          error={this.state.error}
          onRetry={this.reset}
          fullscreen={this.props.fullscreen}
          title={this.props.fullscreen ? "RepoDesk hit an unexpected error" : "This view crashed"}
        />
      );
    }
    return this.props.children;
  }
}

function sameKeys(a?: unknown[], b?: unknown[]): boolean {
  if (a === b) return true;
  if (!a || !b || a.length !== b.length) return false;
  return a.every((value, i) => Object.is(value, b[i]));
}
