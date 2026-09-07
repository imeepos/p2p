// 工作区新增表单（内联三字段）：提交走 admin POST，错误经 workspaceErrorKey 映射。
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { addWorkspace, WorkspaceAdminError } from "@/acp/share-admin-client";
import type { AcpWorkspace } from "@/acp/share-admin-client";
import { workspaceErrorKey } from "./workspace-errors";

export interface WorkspaceAddFormProps {
  adminUrl: string;
  adminToken: string;
  onAdded: (row: AcpWorkspace) => void;
  onError: (message: string) => void;
  busy: boolean;
  setBusy: (value: boolean) => void;
}

export function WorkspaceAddForm(props: WorkspaceAddFormProps) {
  const { t } = useTranslation();
  const [id, setId] = useState("");
  const [name, setName] = useState("");
  const [dir, setDir] = useState("");
  const [error, setError] = useState<string | null>(null);

  const submit = async () => {
    setError(null);
    props.setBusy(true);
    try {
      const row = await addWorkspace(props.adminUrl, props.adminToken, {
        id: id.trim(),
        name: name.trim(),
        dir: dir.trim(),
      });
      setId("");
      setName("");
      setDir("");
      props.onAdded(row);
    } catch (err) {
      const key =
        err instanceof WorkspaceAdminError
          ? workspaceErrorKey(err)
          : "acpManage.workspaces.loadFailed";
      setError(t(key));
      props.onError(t(key));
    } finally {
      props.setBusy(false);
    }
  };

  return (
    <form
      className="flex flex-col gap-2 rounded-md border p-2"
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
      data-testid="acp-ws-add-form"
    >
      <div className="flex flex-col gap-2 sm:flex-row">
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <Label htmlFor="acp-ws-add-id">{t("acpManage.workspaces.idLabel")}</Label>
          <Input id="acp-ws-add-id" value={id} onChange={(e) => setId(e.target.value)} maxLength={64} />
        </div>
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <Label htmlFor="acp-ws-add-name">{t("acpManage.workspaces.nameLabel")}</Label>
          <Input id="acp-ws-add-name" value={name} onChange={(e) => setName(e.target.value)} />
        </div>
      </div>
      <div className="flex flex-col gap-1">
        <Label htmlFor="acp-ws-add-dir">{t("acpManage.workspaces.dirLabel")}</Label>
        <div className="flex gap-2">
          <Input
            id="acp-ws-add-dir"
            value={dir}
            onChange={(e) => setDir(e.target.value)}
            placeholder="/Users/me/projects/demo"
            className="font-mono text-xs"
          />
          <Button type="submit" size="sm" disabled={props.busy} data-testid="acp-ws-add-submit">
            {t("acpManage.workspaces.submit")}
          </Button>
        </div>
      </div>
      {error ? (
        <p className="text-destructive text-xs" role="alert" data-testid="acp-ws-add-error">
          {error}
        </p>
      ) : null}
    </form>
  );
}