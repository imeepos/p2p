// 权限到达提醒桥：store 侧登记新权限时置 notice（seq 自增），这里转 sonner toast。
// store 不直接依赖 toast：桥组件集中处理文案与去重，测试可 mock sonner 断言。
import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";

import { useAcpStore } from "@/acp/acp-store";

const NOTICE_DURATION_MS = 8_000;

export function PermissionNoticeBridge() {
  const { t } = useTranslation();
  const notice = useAcpStore((s) => s.permissionNotice);
  const lastSeqRef = useRef(0);

  useEffect(() => {
    if (!notice || notice.seq === lastSeqRef.current) return;
    lastSeqRef.current = notice.seq;
    toast(t("acp.permission.arrivedToast", { session: notice.sessionId }), {
      description: notice.title,
      duration: NOTICE_DURATION_MS,
    });
  }, [notice, t]);

  return null;
}
