import { useIsFetching, useIsMutating } from "@tanstack/react-query";

// A thin indeterminate progress bar pinned to the top of the window, shown
// whenever any React Query fetch or mutation is in flight. It complements
// StartupSkeleton (which only covers the initial boot) so every later refresh,
// route, run, or save gives the user a single, consistent "working…" signal.
export function GlobalLoader() {
  const fetching = useIsFetching();
  const mutating = useIsMutating();
  const active = fetching + mutating > 0;

  return (
    <div
      className={`global-loader${active ? " global-loader--active" : ""}`}
      role="progressbar"
      aria-hidden={!active}
      aria-busy={active}
      aria-label={active ? "Working" : undefined}
    >
      <div className="global-loader-bar" />
    </div>
  );
}
