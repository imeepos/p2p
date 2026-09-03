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
    {/* Boundary 必须在最外层：module 求值后首个渲染的任何一层（含 ThemeProvider）
        崩溃都要落到兜底 UI，而不是白屏 */}
    <ErrorBoundary>
      <ThemeProvider>
        <ConfirmProvider>
          <App />
          <AppToaster position="top-right" />
        </ConfirmProvider>
      </ThemeProvider>
    </ErrorBoundary>
  </StrictMode>,
);

// 渲染树之外的失败路径可观测信号：脚本错误与未处理 Promise 拒绝
// 是 ErrorBoundary 兜不住的整树崩溃源，禁止静默丢失。
window.addEventListener("error", (event) => {
  console.error("[boot] script error:", event.error ?? event.message);
});
window.addEventListener("unhandledrejection", (event) => {
  console.error("[boot] unhandled rejection:", event.reason);
});
