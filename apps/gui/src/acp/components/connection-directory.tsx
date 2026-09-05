import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import type { I18nKey } from "@/i18n/types";
import { useAcpStore } from "@/acp/acp-store";
import { groupBySource, type AcpScope, type DirectoryEntry } from "@/acp/directory-model";
import { StatusBadge, type StatusTone } from "@/views/shared/status-badge";
import { EmptyState } from "@/views/shared/empty-state";
import { Users } from "lucide-react";

const SCOPE_KEY: Record<AcpScope, I18nKey> = {
  sandbox: "acp.directory.scope.sandbox",
  workspace: "acp.directory.scope.workspace",
  owner: "acp.directory.scope.owner",
};

const SCOPE_TONE: Record<AcpScope, StatusTone> = {
  sandbox: "neutral",
  workspace: "warning",
  owner: "danger",
};

function SourceBadge({ source }: { source: DirectoryEntry["source"] }) {
  const { t } = useTranslation();
  return (
    <StatusBadge tone={source === "discovered" ? "success" : "neutral"} dot={source === "discovered"}>
      {t(source === "discovered" ? "acp.directory.discovered" : "acp.directory.manual")}
    </StatusBadge>
  );
}

function DirectoryRow({ entry }: { entry: DirectoryEntry }) {
  const { t } = useTranslation();
  const setDraft = useAcpStore((s) => s.setDraft);
  const remove = useAcpStore((s) => s.removeDirectoryEntry);
  const scopeHint = t("acp.directory.scopeHint");
  return (
    <div className="flex items-center justify-between gap-2 rounded-md border px-2 py-1.5"
      data-testid={"acp-directory-row-" + entry.peer}>
      <button type="button" className="min-w-0 flex-1 text-left"
        onClick={() => setDraft({ peer: entry.peer })}
        title={t("acp.directory.fill")}
        data-testid={"acp-directory-fill-" + entry.peer}>
        <p className="truncate text-sm font-medium">{entry.name ?? entry.peer}</p>
        {entry.addrs.length > 0 ? (
          <p className="text-muted-foreground truncate text-xs">{entry.addrs[0]}</p>
        ) : null}
      </button>
      <div className="flex items-center gap-1.5">
        <SourceBadge source={entry.source} />
        {/* scope 只读：真实授权权威在桥策略表，GUI 侧不提供切换假操纵杆 */}
        <span data-testid={"acp-directory-scope-badge-" + entry.peer}>
          <StatusBadge tone={SCOPE_TONE[entry.scope]} title={scopeHint}>
            {t(SCOPE_KEY[entry.scope])}
          </StatusBadge>
        </span>
        <Button size="icon" variant="ghost" className="size-7"
          onClick={() => remove(entry.peer)}
          aria-label={t("acp.directory.remove")}
          data-testid={"acp-directory-remove-" + entry.peer}>
          ×
        </Button>
      </div>
    </div>
  );
}

/** 信息架构分组：发现（rendezvous/mDNS）与手动/保存两组；组空不渲染，行序保持 store 顺序 */
function DirectoryGroup(props: { testId: string; title: string; entries: DirectoryEntry[] }) {
  if (props.entries.length === 0) return null;
  return (
    <section className="flex flex-col gap-1" data-testid={props.testId}>
      <h4 className="text-muted-foreground px-2 text-xs font-medium">
        {props.title}（{props.entries.length}）
      </h4>
      {props.entries.map((entry) => (
        <DirectoryRow key={entry.peer} entry={entry} />
      ))}
    </section>
  );
}

/** 连接目录：发现清单（console discovery 契约）与手动 PeerId 分两组呈现 + scope 徽章。
 *  条目点击回填连接表单；scope 改动仅 GUI 侧记录（授权权威在桥策略表）。 */
export function ConnectionDirectory() {
  const { t } = useTranslation();
  const [peer, setPeer] = useState("");
  const [invalid, setInvalid] = useState(false);
  const directory = useAcpStore((s) => s.directory);
  const addManualPeer = useAcpStore((s) => s.addManualPeer);
  const grouped = groupBySource(directory);

  const submit = () => {
    if (!peer.trim()) {
      setInvalid(true);
      return;
    }
    setInvalid(false);
    addManualPeer(peer);
    setPeer("");
  };

  return (
    <Card className="flex flex-col" data-testid="acp-directory-card">
      <CardHeader className="pb-2">
        <CardTitle className="text-base">{t("acp.directory.card")}</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        <div className="flex gap-2">
          <Input value={peer}
            onChange={(e) => {
              setPeer(e.target.value);
              setInvalid(false);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                submit();
              }
            }}
            placeholder={t("acp.directory.addPlaceholder")}
            aria-label={t("acp.directory.addPlaceholder")}
            data-testid="acp-directory-input" />
          <Button variant="outline" onClick={submit} data-testid="acp-directory-add">
            {t("acp.directory.add")}
          </Button>
        </div>
        {invalid ? (
          <p className="text-destructive text-xs" role="alert" data-testid="acp-directory-error">
            {t("acp.directory.invalidPeer")}
          </p>
        ) : null}
        {directory.length === 0 ? (
          <EmptyState
            icon={Users}
            title={t("acp.directory.empty")}
            description={t("acp.directory.emptyHint")}
          />
        ) : (
          <div className="flex flex-col gap-3">
            <DirectoryGroup
              testId="acp-directory-group-discovered"
              title={t("acp.directory.groupDiscovered")}
              entries={grouped.discovered}
            />
            <DirectoryGroup
              testId="acp-directory-group-manual"
              title={t("acp.directory.groupManual")}
              entries={grouped.manual}
            />
            <p className="text-muted-foreground px-2 text-xs" data-testid="acp-directory-scope-hint">
              {t("acp.directory.scopeHint")}
            </p>
          </div>
        )}
      </CardContent>
    </Card>
  );
}