import { Component, type ReactNode } from "react";

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  error: Error | null;
}

// 全局渲染兜底：捕获子树渲染异常，避免整窗白屏无提示。
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error): void {
    console.error("[ErrorBoundary] 渲染异常：", error);
  }

  private reset = (): void => {
    this.setState({ error: null });
  };

  render(): ReactNode {
    if (this.state.error !== null) {
      return (
        <div className="flex h-screen flex-col items-center justify-center gap-4 bg-background p-8 text-foreground">
          <p className="text-lg font-semibold">界面出错了 / Something went wrong</p>
          <p className="max-w-md break-all text-center text-sm text-muted-foreground">
            {String(this.state.error)}
          </p>
          <button
            type="button"
            onClick={this.reset}
            className="rounded-md bg-primary px-4 py-2 text-sm text-primary-foreground"
          >
            重试 / Retry
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
