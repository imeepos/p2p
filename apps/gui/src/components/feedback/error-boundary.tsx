import { Component, type ReactNode } from "react";

import i18n from "@/i18n";
import { goToHomeRoute, reloadWindow } from "./window-actions";

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  error: Error | null;
}

const outlineButtonClass =
  "rounded-md border border-border px-4 py-2 text-sm text-foreground hover:bg-accent";

// 全局渲染兜底：捕获子树渲染异常，避免整窗白屏无提示。
// 挂载点在 Router 之外，出路按钮必须不经 Router 即可用：重试之外提供
// 返回首页（复位路由）与重载窗口（确定性崩溃时重试无效的死路出路）。
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error): void {
    console.error("[ErrorBoundary] 渲染异常：", error);
  }

  private retry = (): void => {
    this.setState({ error: null });
  };

  // 返回首页 = 复位路由 + 复位兜底状态，子树在首页路由下重新渲染
  private goHome = (): void => {
    goToHomeRoute();
    this.setState({ error: null });
  };

  render(): ReactNode {
    if (this.state.error !== null) {
      return (
        <div
          role="alert"
          className="flex h-screen flex-col items-center justify-center gap-4 bg-background p-8 text-foreground"
        >
          <p className="text-lg font-semibold">
            {i18n.t("common.errorBoundary.title")}
          </p>
          <p className="max-w-md break-all text-center text-sm text-muted-foreground">
            {String(this.state.error)}
          </p>
          <div className="flex items-center gap-3">
            <button type="button" onClick={this.goHome} className={outlineButtonClass}>
              {i18n.t("common.errorBoundary.goHome")}
            </button>
            <button type="button" onClick={reloadWindow} className={outlineButtonClass}>
              {i18n.t("common.errorBoundary.reload")}
            </button>
            <button
              type="button"
              onClick={this.retry}
              className="rounded-md bg-primary px-4 py-2 text-sm text-primary-foreground"
            >
              {i18n.t("common.errorBoundary.retry")}
            </button>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}
