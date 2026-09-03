import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "@/i18n";
import App from "./App";
import { ConfirmProvider } from "./components/feedback/confirm-provider";
import { ErrorBoundary } from "./components/feedback/error-boundary";
import { AppToaster } from "./components/ui/sonner";
import { ThemeProvider } from "./theme/theme-provider";
import "./index.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ThemeProvider>
      <ErrorBoundary>
        <ConfirmProvider>
          <App />
          <AppToaster position="top-right" />
        </ConfirmProvider>
      </ErrorBoundary>
    </ThemeProvider>
  </StrictMode>,
);
