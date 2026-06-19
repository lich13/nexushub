import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import React, { Component, ErrorInfo, ReactNode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./styles.css";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchInterval: 15000,
      retry: 1
    }
  }
});

class RootErrorBoundary extends Component<
  { children: ReactNode },
  { error: Error | null }
> {
  state: { error: Error | null } = { error: null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    try {
      localStorage.setItem("nexushub.lastRenderError", error.message);
    } catch {
      // ignore storage failures
    }
    console.error("NexusHub render failed", error, info.componentStack);
  }

  render() {
    if (!this.state.error) {
      return this.props.children;
    }
    return (
      <main className="fatal-screen">
        <section className="panel wide-panel">
          <header><strong>界面载入失败</strong></header>
          <div className="form-error">{this.state.error.message || "未知错误"}</div>
        </section>
      </main>
    );
  }
}

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <RootErrorBoundary>
        <App />
      </RootErrorBoundary>
    </QueryClientProvider>
  </React.StrictMode>
);
