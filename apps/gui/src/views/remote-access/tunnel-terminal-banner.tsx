import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import type { I18nKey } from "@/i18n/types";

// 开隧道/serve 操作终态横幅入参：成功带链接与对端 peer，失败带人话原因
// 与六值闭集错误码（可空）。页面层持有状态，卡片经回调上抛。
export interface TunnelBannerInput {
  tone: "success" | "error";
  reason?: string;
  code?: string | null;
  link?: string | null;
  peer?: string | null;
  target?: string | null;
}

// 六值闭集 → 人话键映射；闭集外码（防御）回退原码展示。
const CODE_KEYS: Record<string, I18nKey> = {
  bad_ticket: "remoteAccess.tunnel.code.bad_ticket",
  target_not_allowed: "remoteAccess.tunnel.code.target_not_allowed",
  busy: "remoteAccess.tunnel.code.busy",
  dial_failed: "remoteAccess.tunnel.code.dial_failed",
  io: "remoteAccess.tunnel.code.io",
  shutdown: "remoteAccess.tunnel.code.shutdown",
};

function codeLabel(code: string, t: (key: I18nKey) => string): string {
  const key = CODE_KEYS[code];
  return key ? t(key) : code;
}

// 终态横幅（gap-matrix §4 纯前端栏 S7）：role=alert 既有盒样式；错误态
// 沿用红色边框错误盒口径，成功态同构默认边框。可关闭（知道了）。
export function TunnelTerminalBanner({
  banner,
  onDismiss,
}: {
  banner: TunnelBannerInput;
  onDismiss: () => void;
}) {
  const { t } = useTranslation();
  const isError = banner.tone === "error";
  return (
    <div
      role="alert"
      data-testid="tunnel-terminal-banner"
      data-tone={banner.tone}
      className={
        isError
          ? "text-destructive space-y-1 rounded-md border border-red-300 p-3 text-sm"
          : "space-y-1 rounded-md border p-3 text-sm"
      }
    >
      <p className="font-medium">
        {isError
          ? t("remoteAccess.tunnel.bannerFailTitle")
          : t("remoteAccess.tunnel.bannerSuccessTitle")}
      </p>
      {banner.target ? (
        <p data-testid="tunnel-banner-target">
          {t("remoteAccess.tunnel.bannerTarget")}
          ：<span className="font-mono">{banner.target}</span>
        </p>
      ) : null}
      {banner.link ? (
        <p data-testid="tunnel-banner-link">
          {t("remoteAccess.status.openUrl")}
          ：<span className="font-mono break-all">{banner.link}</span>
        </p>
      ) : null}
      {banner.peer ? (
        <p data-testid="tunnel-banner-peer">
          {t("remoteAccess.tunnel.bannerPeer")}
          ：<span className="font-mono break-all">{banner.peer}</span>
        </p>
      ) : null}
      {banner.reason ? <p>{banner.reason}</p> : null}
      {banner.code ? (
        <p data-testid="tunnel-banner-code">
          {t("remoteAccess.tunnel.bannerCode")}
          ：<span className="font-mono">{banner.code}</span>（
          {codeLabel(banner.code, t)}）
        </p>
      ) : null}
      <Button type="button" size="sm" variant="outline" onClick={onDismiss}>
        {t("remoteAccess.error.dismiss")}
      </Button>
    </div>
  );
}
