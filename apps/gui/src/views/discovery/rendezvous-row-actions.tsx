import { useTranslation } from "react-i18next";

import { toastError, toastSuccess } from "@/components/feedback/toast";
import { AsyncButton } from "@/components/feedback/async-button";
import type { I18nKey } from "@/i18n/types";

// rendezvous 手动注册/查询：契约暂无对应命令，本地 toast 反馈（集成波接真命令）。
export function RendezvousRowActions({ addr, running }: { addr: string; running: boolean }) {
  const { t } = useTranslation();

  const run = async (kind: "register" | "query") => {
    if (!running) return;
    const key: I18nKey = kind === "register"
      ? "discovery.rendezvous.registerSent"
      : "discovery.rendezvous.querySent";
    toastSuccess(t(key, { addr }));
  };

  const button = (kind: "register" | "query") => (
    <AsyncButton
      type="button"
      variant="outline"
      size="sm"
      disabled={!running}
      action={() => run(kind)}
      onError={(error) => {
        console.error("[discovery] rendezvous 手动操作失败", error);
        toastError(t("discovery.rendezvous.actionFailed"));
      }}
    >
      {t(
        (kind === "register"
          ? "discovery.rendezvous.register"
          : "discovery.rendezvous.query") as I18nKey,
      )}
    </AsyncButton>
  );

  return (
    <>
      {button("register")}
      {button("query")}
    </>
  );
}
