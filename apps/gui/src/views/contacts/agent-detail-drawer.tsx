import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { MessagesSquareIcon, TriangleAlertIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/monitor/sheet";
import { useAcpStore } from "@/acp/acp-store";
import {
  forgetEndpointMeta,
  policyOf,
  useEndpointMetaStore,
} from "@/acp/endpoint-meta";
import type { AcpEndpoint } from "@/acp/protocol";
import { toastInfo } from "@/components/feedback/toast";
import { useConfirm } from "@/components/feedback/confirm-provider";
import { wsHostOf } from "@/lib/conversation-entry";
import { errorText } from "@/views/shared/form-flow";

import { CapabilitiesCard } from "./capabilities-card";
import { ConfigPanel } from "./config-panel";
import { PermissionPanel } from "./permission-panel";
import { DrawerConnection } from "./drawer-connection";
import { PolicyEditor } from "./policy-editor";

function Block({ id, title, children }: { id: string; title: string; children: React.ReactNode }) {
  return (
    <section className="flex flex-col gap-2" data-testid={"contacts-drawer-block-" + id}>
      <h3 className="text-sm font-semibold">{title}</h3>
      {children}
    </section>
  );
}

// agent 管理详情抽屉（§3.3）：右滑五块——连接 / 能力 / 权限策略 / 会话 /
// 危险区。停用第三档单次确认；删除第二档确认并明示「将同时移除本地会
// 话记录索引」。
export function AgentDetailDrawer({
  endpointId,
  onOpenChange,
}: {
  endpointId: string | null;
  onOpenChange: (open: boolean) => void;
}) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const confirm = useConfirm();
  const saved = useAcpStore((s) => s.saved);
  const phase = useAcpStore((s) => s.phase);
  const activeEndpointId = useAcpStore((s) => s.activeEndpointId);
  const sessions = useAcpStore((s) => s.sessions);
  const resumeSession = useAcpStore((s) => s.resumeSession);
  const upsertSaved = useAcpStore((s) => s.upsertSaved);
  const removeSavedById = useAcpStore((s) => s.removeSavedById);
  const disabled = useEndpointMetaStore((s) => (endpointId ? s.disabled[endpointId] === true : false));
  const policy = policyOf(endpointId);

  const endpoint: AcpEndpoint | null =
    saved.find((e) => (e.endpointId ?? e.wsUrl) === endpointId) ?? null;
  const isActive = endpointId !== null && endpointId === activeEndpointId;

  const setDisabled = (value: boolean) => {
    if (endpointId) useEndpointMetaStore.getState().setDisabled(endpointId, value);
  };

  const disable = async () => {
    const ok = await confirm({
      title: t("contacts.agents.disable"),
      description: t("contacts.agents.disabledHint"),
      confirmText: t("contacts.agents.disable"),
      cancelText: t("common.actions.cancel"),
      destructive: true,
    });
    if (ok) setDisabled(true);
  };

  const remove = async () => {
    const ok = await confirm({
      title: t("contacts.agents.removeConfirmTitle"),
      description: t("contacts.agents.removeConfirmDescription", {
        name: endpoint?.alias || (endpoint ? wsHostOf(endpoint.wsUrl) : "") || (endpointId ?? ""),
      }),
      confirmText: t("contacts.agents.removeConfirmAction"),
      cancelText: t("common.actions.cancel"),
      destructive: true,
    });
    if (!ok || !endpointId) return;
    removeSavedById(endpointId);
    forgetEndpointMeta(endpointId);
    onOpenChange(false);
  };

  const openSession = async (sessionId: string) => {
    if (isActive && phase === "online") {
      try {
        await resumeSession(sessionId);
      } catch (error) {
        // P3#18 已跳转会话页但恢复失败：中性提示说明降级，不静默
        console.error("[contacts] 恢复会话失败，跳转会话页继续", sessionId, error);
        toastInfo(t("contacts.agents.resumeDegraded"), errorText(error));
      }
    }
    navigate("/chat?agent=" + endpointId);
    onOpenChange(false);
  };

  const title = endpoint
    ? endpoint.alias || wsHostOf(endpoint.wsUrl) || endpointId
    : (endpointId ?? "");

  return (
    <Sheet open={endpointId !== null} onOpenChange={onOpenChange}>
      <SheetContent className="overflow-y-auto" data-testid="contacts-agent-drawer">
        <SheetHeader>
          <SheetTitle data-testid="contacts-agent-drawer-title">{title}</SheetTitle>
          <SheetDescription className="font-mono">
            {endpoint ? endpoint.wsUrl : ""}
          </SheetDescription>
        </SheetHeader>
        {endpoint && endpointId ? (
          <div className="flex flex-col gap-4">
            <Block id="connection" title={t("contacts.drawer.connection")}>
              <DrawerConnection
                endpoint={endpoint}
                isActive={isActive}
                onSave={(next) => upsertSaved(next)}
              />
              {disabled ? (
                <p className="text-warning text-xs" data-testid="contacts-agent-drawer-disabled">
                  {t("contacts.drawer.disabledHint")}
                </p>
              ) : null}
            </Block>
            <Separator />
            <Block id="capabilities" title={t("contacts.drawer.capabilities")}>
              <CapabilitiesCard />
              <ConfigPanel />
            </Block>
            <Separator />
            <Block id="policy" title={t("contacts.drawer.policy")}>
              <PolicyEditor
                policy={policy}
                onChange={(next) => useEndpointMetaStore.getState().setPolicy(endpointId, next)}
              />
              <PermissionPanel />
            </Block>
            <Separator />
            <Block id="sessions" title={t("contacts.drawer.sessions")}>
              {isActive && sessions.length > 0 ? (
                <div className="flex flex-col gap-1">
                  {sessions.map((session) => (
                    <button
                      key={session.sessionId}
                      type="button"
                      className="hover:bg-accent flex items-center gap-2 rounded-md border px-2 py-1.5 text-left"
                      onClick={() => void openSession(session.sessionId)}
                      data-testid={"contacts-agent-session-" + session.sessionId}
                    >
                      <MessagesSquareIcon aria-hidden className="size-4 shrink-0" />
                      {/* P3#17 裸 sessionId 等宽缩小 + hover 全文 */}
                      <span
                        className={session.title ? "truncate text-sm" : "truncate font-mono text-xs"}
                        title={session.title ?? session.sessionId}
                      >
                        {session.title ?? session.sessionId}
                      </span>
                    </button>
                  ))}
                </div>
              ) : (
                <p className="text-muted-foreground text-xs" data-testid="contacts-agent-sessions-empty">
                  {t("contacts.drawer.sessionEmpty")}
                </p>
              )}
            </Block>
            <Separator />
            <Block id="dangerZone" title={t("contacts.drawer.dangerZone")}>
              <p className="text-muted-foreground text-xs">{t("contacts.drawer.dangerHint")}</p>
              <div className="flex items-center gap-2">
                {disabled ? (
                  <Button type="button" variant="outline" size="sm" onClick={() => setDisabled(false)} data-testid="contacts-agent-drawer-enable">
                    {t("contacts.agents.enable")}
                  </Button>
                ) : (
                  <Button type="button" variant="outline" size="sm" onClick={() => void disable()} data-testid="contacts-agent-drawer-disable">
                    <TriangleAlertIcon aria-hidden className="size-4" />
                    {t("contacts.agents.disable")}
                  </Button>
                )}
                <Button type="button" variant="destructive" size="sm" onClick={() => void remove()} data-testid="contacts-agent-drawer-remove">
                  {t("contacts.agents.removeConfirmAction")}
                </Button>
              </div>
            </Block>
          </div>
        ) : null}
      </SheetContent>
    </Sheet>
  );
}
