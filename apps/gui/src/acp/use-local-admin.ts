// 本机 agent 管理端点自动发现（2026-09-07 用户裁决：分享流免手填 admin token）：
// 数据源 = acp-agent 落盘的自描述文件（src-tauri acp_local_descriptor 读
// ~/.dsh/acp/local-agent.json）。仅当无已登记管理端点时启用查询，挂载期一次；
// 读取失败显式告警不静默，done 标志供调用方消除引导文案闪烁。

import { useEffect, useState } from "react";

import { ipc } from "@/lib/ipc";

import type { AdminEndpointRef } from "./admin-endpoints";

export function useLocalAdminCandidate(
  enabled: boolean,
  label: string,
): { candidate: AdminEndpointRef | null; done: boolean } {
  const [candidate, setCandidate] = useState<AdminEndpointRef | null>(null);
  const [done, setDone] = useState(false);
  // enabled false→true（弹层重开）时丢弃陈旧候选：渲染期状态调整，不落 effect
  const [armedBefore, setArmedBefore] = useState(enabled);
  if (armedBefore !== enabled) {
    setArmedBefore(enabled);
    if (enabled) {
      setCandidate(null);
      setDone(false);
    }
  }

  useEffect(() => {
    if (!enabled) return;
    let dead = false;
    ipc
      .acpLocalDescriptor()
      .then((descriptor) => {
        if (dead) return;
        setCandidate(
          descriptor?.adminUrl && descriptor.token
            ? { id: descriptor.adminUrl, label, url: descriptor.adminUrl, token: descriptor.token }
            : null,
        );
        setDone(true);
      })
      .catch((error) => {
        console.warn("[acp] 本机 agent 描述读取失败", error);
        if (!dead) setDone(true);
      });
    return () => {
      dead = true;
    };
    // label 随 i18n 变化无需重查：candidate.label 以首次查询为准
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled]);

  return { candidate, done };
}
