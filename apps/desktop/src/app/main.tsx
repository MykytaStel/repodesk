import React from "react";
import ReactDOM from "react-dom/client";
import {
  MutationCache,
  QueryCache,
  QueryClient,
  QueryClientProvider,
} from "@tanstack/react-query";
import App from "./App";
import { ErrorBoundary } from "../shared/ui/ErrorBoundary";
import { ToastProvider } from "../shared/ui/Toast";
import { reportError } from "../shared/utils/errors";

// Every query/mutation failure flows through the central reporter (console +
// Debug tab + event journal), so no command error is silently swallowed.
const queryClient = new QueryClient({
  queryCache: new QueryCache({
    onError: (error, query) =>
      reportError(`query:${String(query.queryKey?.[0] ?? "unknown")}`, error),
  }),
  mutationCache: new MutationCache({
    onError: (error) => reportError("mutation", error),
  }),
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: false, // Prevent excessive refetching
      staleTime: 5000,
      retry: false,
    },
  },
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary scope="app" fullscreen>
      <QueryClientProvider client={queryClient}>
        <ToastProvider>
          <App />
        </ToastProvider>
      </QueryClientProvider>
    </ErrorBoundary>
  </React.StrictMode>,
);
