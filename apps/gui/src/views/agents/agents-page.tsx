// /agents 智能体页（设计 §8，消息页同构）：header + SegmentedControl
// [发现(n)][我的] + 分区列表 + 行内动作。数据面：发现 = console WS proto=a2a
// 卡片通道（契约 §17.1）；我的 = 本机 agent admin CRUD（§17.2，凭据 AcpLocalDescriptor）。
// 本机 agent 未就绪（descriptor null）显式降级提示，不误报空态语义。
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { SegmentedControl } from "@/components/ui/segmented-control";
import { toastError, toastSuccess } from "@/components/feedback/toast";
import { ipc } from "@/lib/ipc";

import { useAgentsStore } from "@/a2a/agents-store";
import type { AgentCreateInput, AgentDefJson } from "@/a2a/types";

import { CreateAgentDialog } from "./create-agent-dialog";
import { DiscoverSection } from "./discover-section";
import { MineSection } from "./mine-section";

type AgentsView = "discover" | "mine";

function useAgentsWiring(): { localReady: boolean } {
  const connectCards = useAgentsStore((s) => s.connectCards);
  const disconnectCards = useAgentsStore((s) => s.disconnectCards);
  const loadMine = useAgentsStore((s) => s.loadMine);
  const [localReady, setLocalReady] = useState(false);

  useEffect(() => {
    let dead = false;
    // 描述符（adminUrl/token/宿主 peer）与 console 连接面（wsUrl/token）双查询，
    // 任一缺失挂起接入并显式留痕（console.warn），绝不静默空转。
    void Promise.all([ipc.acpLocalDescriptor(), ipc.acpConsoleStatus()])
      .then(([descriptor, status]) => {
        if (dead) return;
        if (!descriptor) {
          console.warn("[a2a] 本机 agent 描述不可用：/agents 数据面挂起");
          setLocalReady(false);
          return;
        }
        setLocalReady(true);
        void loadMine(descriptor.adminUrl, descriptor.token);
        if (status.phase === "connected" && status.wsUrl && status.token) {
          connectCards(status.wsUrl, status.token, descriptor.peer);
        } else {
          console.warn("[a2a] console 未 connected：卡片通道挂起（状态到达后重进页面）");
        }
      })
      .catch((error) => console.warn("[a2a] 数据面接入失败", error));
    return () => {
      dead = true;
      disconnectCards();
    };
  }, [connectCards, disconnectCards, loadMine]);

  return { localReady };
}

/** admin 动作统一出口：成功 toast，失败原文上浮（返回 ok 驱动对话框关闭）。 */
async function runMineAction(action: () => Promise<boolean>, okMessage: string): Promise<boolean> {
  const ok = await action();
  if (ok) toastSuccess(okMessage);
  return ok;
}

export function AgentsPage() {
  const { t } = useTranslation();
  const [view, setView] = useState<AgentsView>("discover");
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editing, setEditing] = useState<AgentDefJson | null>(null);
  const discovered = useAgentsStore((s) => s.discovered);
  const mine = useAgentsStore((s) => s.mine);
  const channelError = useAgentsStore((s) => s.channelError);
  const lastActionError = useAgentsStore((s) => s.lastActionError);
  const createMine = useAgentsStore((s) => s.createMine);
  const updateVisibility = useAgentsStore((s) => s.updateVisibility);
  const unpublish = useAgentsStore((s) => s.unpublish);
  const { localReady } = useAgentsWiring();

  // 失败上浮：通道 error 帧与 admin 动作失败原文（toastError，禁静默）
  useEffect(() => {
    if (channelError) toastError(channelError);
  }, [channelError]);
  useEffect(() => {
    if (lastActionError) toastError(lastActionError);
  }, [lastActionError]);

  const openCreate = (): void => {
    setEditing(null);
    setDialogOpen(true);
  };
  const openEdit = (def: AgentDefJson): void => {
    setEditing(def);
    setDialogOpen(true);
  };

  const confirmDialog = async (input: AgentCreateInput): Promise<boolean> => {
    const descriptor = await ipc.acpLocalDescriptor();
    if (!descriptor) return false;
    if (editing) {
      return runMineAction(
        () =>
          updateVisibility(descriptor.adminUrl, descriptor.token, editing.agentId, input.visibility),
        t("agents.toast.updated"),
      );
    }
    return runMineAction(
      () => createMine(descriptor.adminUrl, descriptor.token, input),
      t("agents.toast.created"),
    );
  };

  const handleUnpublish = async (def: AgentDefJson): Promise<unknown> => {
    const descriptor = await ipc.acpLocalDescriptor();
    if (!descriptor) return false;
    return runMineAction(
      () => unpublish(descriptor.adminUrl, descriptor.token, def.agentId),
      t("agents.toast.unpublished"),
    );
  };

  return (
    <section data-testid="agents-page" className="flex min-h-0 flex-1 flex-col gap-4">
      <header className="flex flex-wrap items-start justify-between gap-2">
        <div className="flex flex-col gap-1">
          <h1 className="text-lg font-semibold tracking-tight">{t("agents.title")}</h1>
          <p className="text-muted-foreground text-sm">{t("agents.description")}</p>
        </div>
        <SegmentedControl
          value={view}
          onChange={setView}
          ariaLabel={t("agents.title")}
          options={[
            { value: "discover", label: t("agents.view.discover"), count: discovered.length },
            { value: "mine", label: t("agents.view.mine") },
          ]}
        />
      </header>
      {!localReady ? (
        <p className="text-muted-foreground rounded-md border border-dashed p-4 text-center text-sm" data-testid="agents-local-not-ready">
          {t("agents.err.loadFailed")}
        </p>
      ) : null}
      <div className="scroll-slim flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto pb-4">
        {view === "discover" ? (
          <DiscoverSection rows={discovered} />
        ) : (
          <MineSection
            mine={mine}
            onEdit={openEdit}
            onCreate={openCreate}
            onUnpublish={handleUnpublish}
          />
        )}
      </div>
      <CreateAgentDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        editing={editing}
        onConfirm={confirmDialog}
      />
    </section>
  );
}
