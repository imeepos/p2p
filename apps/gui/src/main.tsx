import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "@/i18n";
import App from "./App";
import { ConfirmProvider } from "./components/feedback/confirm-provider";
import { ErrorBoundary } from "./components/feedback/error-boundary";
import { AppToaster } from "./components/ui/sonner";
import { ThemeProvider } from "./theme/theme-provider";
import { installAgentBridge } from "./lib/agent-bridge";
import { installControlBridge } from "./lib/control-bridge";
import { installDataWatch } from "./lib/data-watch-install";
import { installErrorReport } from "./lib/error-report";
import { installPageBridge } from "./lib/page-bridge";
import "./index.css";

// 错误感知管线先于渲染安装：白屏/渲染异常也要进 frontend.log（契约 §8）。
installErrorReport();
installAgentBridge();
// GC1 控制通道：hash 路由实时上报（health 的 route 字段来源）
installControlBridge();
// GC3 页面语义协议：/page/* 端点经 __P2P_PAGES__ 桥驱动页面注册表
installPageBridge();
// W1 数据目录实时感知：CLI 写入 data-changed → 定向重载对应 store
installDataWatch();

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
