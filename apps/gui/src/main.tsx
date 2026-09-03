import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "@/i18n";
import App from "./App";
import { ConfirmProvider } from "./components/feedback/confirm-provider";
import { ErrorBoundary } from "./components/feedback/error-boundary";
import { AppToaster } from "./components/ui/sonner";
import { ThemeProvider } from "./theme/theme-provider";
import { installAgentBridge } from "./lib/agent-bridge";
import { installErrorReport } from "./lib/error-report";
import "./index.css";

// 错误感知管线先于渲染安装：白屏/渲染异常也要进 frontend.log（契约 §8）。
installErrorReport();
installAgentBridge();

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
