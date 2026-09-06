import { useEffect } from "react";

import { useNodeStore } from "@/stores/node-store";

// UX1 启动即在线：引导完成且首次状态取回成功后，节点未运行则自动启动。
// 一次性闸门与手动停止压制都在 node-store.maybeAutoStart 内；本 hook 只在
// ready + 未运行时触发入口，周期刷新/重新引导的重复进入由闸门拦截。
export function useNodeAutoStart(): void {
  const bootstrapPhase = useNodeStore((s) => s.bootstrapPhase);
  const hasStatus = useNodeStore((s) => s.status !== null);
  const running = useNodeStore((s) => s.status?.running ?? false);
  const maybeAutoStart = useNodeStore((s) => s.maybeAutoStart);

  useEffect(() => {
    if (bootstrapPhase === "ready" && hasStatus && !running) {
      void maybeAutoStart();
    }
  }, [bootstrapPhase, hasStatus, running, maybeAutoStart]);
}
