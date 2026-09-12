import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { PlusIcon, SquarePenIcon, Trash2Icon } from "lucide-react";

import { useConfirm } from "@/components/feedback/confirm-provider";
import { CommandErrorText } from "@/components/feedback/command-error";
import { toastError, toastSuccess } from "@/components/feedback/toast";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { AuthzRoleView } from "@/lib/ipc-types";
import { useAuthzStore } from "@/stores/authz-store";
import { errorText } from "@/views/shared/form-flow";
import { permLabel } from "@/views/contacts/role-perm-labels";
import { RoleEditorFields } from "@/views/contacts/role-editor-fields";

const ROLE_ID_RE = /^[a-z0-9-]{1,32}$/;

type EditorState =
  | { kind: "closed" }
  | { kind: "create" }
  | { kind: "edit"; role: AuthzRoleView };

interface RoleManagerDialogProps {
  onOpenChange: (open: boolean) => void;
}

// 角色列表行：name + builtin 徽章 + 权限标签徽章串；自定义角色才有行内
// 编辑/删除（builtin 行两者皆无，不做禁用灰按钮，红线：内建不可改不可删）。
function RoleRow({
  role,
  busy,
  onEdit,
  onDelete,
}: {
  role: AuthzRoleView;
  busy: boolean;
  onEdit: () => void;
  onDelete: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div
      className="flex flex-col gap-1.5 rounded-md border p-2.5"
      data-testid={"role-row-" + role.roleId}
    >
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium">{role.name}</span>
          {role.builtin ? (
            <Badge variant="secondary">{t("contacts.authz.builtinTag")}</Badge>
          ) : null}
        </div>
        {role.builtin ? null : (
          <div className="flex gap-1">
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={onEdit}
              data-testid={"role-edit-" + role.roleId}
            >
              <SquarePenIcon aria-hidden className="size-4" />
              {t("contacts.authz.manager.edit")}
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="text-destructive hover:text-destructive"
              onClick={onDelete}
              disabled={busy}
              data-testid={"role-delete-" + role.roleId}
            >
              <Trash2Icon aria-hidden className="size-4" />
              {t("contacts.authz.manager.delete")}
            </Button>
          </div>
        )}
      </div>
      <div className="flex flex-wrap gap-1" data-testid={"role-perms-" + role.roleId}>
        {role.permissions.map((permKey) => (
          <Badge key={permKey} variant="outline" className="text-xs font-normal">
            {permLabel(permKey, t)}
          </Badge>
        ))}
      </div>
    </div>
  );
}

// 角色管理对话框（§18 S3 管理面）：列表/新建/编辑三态切换；删除走
// useConfirm 确认；校验失败原位、命令失败原位 + toast 双通道。
export function RoleManagerDialog({ onOpenChange }: RoleManagerDialogProps) {
  const { t } = useTranslation();
  const roles = useAuthzStore((s) => s.roles);
  const permissions = useAuthzStore((s) => s.permissions);
  const loadAll = useAuthzStore((s) => s.loadAll);
  const createRole = useAuthzStore((s) => s.createRole);
  const updateRole = useAuthzStore((s) => s.updateRole);
  const deleteRole = useAuthzStore((s) => s.deleteRole);
  const confirm = useConfirm();

  const [editor, setEditor] = useState<EditorState>({ kind: "closed" });
  const [roleId, setRoleId] = useState("");
  const [name, setName] = useState("");
  const [note, setNote] = useState("");
  const [selected, setSelected] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (roles.length === 0) void loadAll();
  }, [roles.length, loadAll]);

  const openCreate = () => {
    setRoleId("");
    setName("");
    setNote("");
    setSelected([]);
    setError(null);
    setEditor({ kind: "create" });
  };

  const openEdit = (role: AuthzRoleView) => {
    setRoleId(role.roleId);
    setName(role.name);
    setNote(role.note);
    setSelected([...role.permissions]);
    setError(null);
    setEditor({ kind: "edit", role });
  };

  const togglePerm = (permKey: string) => {
    setSelected((prev) =>
      prev.includes(permKey) ? prev.filter((k) => k !== permKey) : [...prev, permKey],
    );
  };

  const submit = async () => {
    if (editor.kind === "closed") return;
    if (editor.kind === "create" && !ROLE_ID_RE.test(roleId)) {
      setError(t("contacts.authz.manager.roleIdInvalid"));
      return;
    }
    const trimmedName = name.trim();
    if (!trimmedName) {
      setError(t("contacts.authz.manager.nameRequired"));
      return;
    }
    if (selected.length === 0) {
      setError(t("contacts.authz.manager.permissionsRequired"));
      return;
    }
    setBusy(true);
    setError(null);
    const trimmedNote = note.trim();
    try {
      if (editor.kind === "create") {
        await createRole(roleId, trimmedName, selected, trimmedNote);
        toastSuccess(t("contacts.authz.manager.createSuccess"));
      } else {
        await updateRole(editor.role.roleId, trimmedName, selected, trimmedNote);
        toastSuccess(t("contacts.authz.manager.updateSuccess"));
      }
      setEditor({ kind: "closed" });
    } catch (err) {
      console.error("[contacts] 角色保存失败", err);
      const detail = errorText(err);
      setError(detail);
      toastError(
        editor.kind === "create"
          ? t("contacts.authz.manager.createFailed")
          : t("contacts.authz.manager.updateFailed"),
        {
          description: detail,
          context: editor.kind === "create" ? "authz_role_create" : "authz_role_update",
        },
      );
    } finally {
      setBusy(false);
    }
  };

  const removeRole = async (role: AuthzRoleView) => {
    const ok = await confirm({
      title: t("contacts.authz.manager.deleteConfirmTitle"),
      description: t("contacts.authz.manager.deleteConfirmDesc", { name: role.name }),
      confirmText: t("contacts.authz.manager.delete"),
      cancelText: t("common.actions.cancel"),
      destructive: true,
    });
    if (!ok) return;
    setBusy(true);
    setError(null);
    try {
      await deleteRole(role.roleId);
      toastSuccess(t("contacts.authz.manager.deleteSuccess"));
    } catch (err) {
      console.error("[contacts] 角色删除失败", err);
      const detail = errorText(err);
      setError(detail);
      toastError(t("contacts.authz.manager.deleteFailed"), {
        description: detail,
        context: "authz_role_delete",
      });
    } finally {
      setBusy(false);
    }
  };

  const dialogTitle =
    editor.kind === "create"
      ? t("contacts.authz.manager.formCreateTitle")
      : editor.kind === "edit"
        ? t("contacts.authz.manager.formEditTitle")
        : t("contacts.authz.manager.title");

  return (
    <Dialog open onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg" data-testid="role-manager-dialog">
        <DialogHeader>
          <DialogTitle>{dialogTitle}</DialogTitle>
          <DialogDescription>
            {editor.kind === "closed" ? t("contacts.authz.manager.description") : null}
          </DialogDescription>
        </DialogHeader>
        {editor.kind === "closed" ? (
          <div className="flex flex-col gap-2" data-testid="role-manager-list">
            {roles.map((role) => (
              <RoleRow
                key={role.roleId}
                role={role}
                busy={busy}
                onEdit={() => openEdit(role)}
                onDelete={() => void removeRole(role)}
              />
            ))}
          </div>
        ) : (
          <div className="flex flex-col gap-3" data-testid="role-manager-form">
            <RoleEditorFields
              isCreate={editor.kind === "create"}
              roleId={roleId}
              onRoleIdChange={setRoleId}
              name={name}
              onNameChange={setName}
              note={note}
              onNoteChange={setNote}
              permissions={permissions}
              selected={selected}
              onToggle={togglePerm}
            />
          </div>
        )}
        {error ? <CommandErrorText message={error} testId="role-manager-error" /> : null}
        {editor.kind === "closed" ? (
          <Button
            type="button"
            variant="outline"
            onClick={openCreate}
            data-testid="role-manager-new"
          >
            <PlusIcon aria-hidden className="size-4" />
            {t("contacts.authz.manager.newRole")}
          </Button>
        ) : (
          <div className="flex justify-end gap-2">
            <Button
              type="button"
              variant="outline"
              onClick={() => setEditor({ kind: "closed" })}
              data-testid="role-manager-back"
            >
              {t("contacts.authz.manager.back")}
            </Button>
            <Button
              type="button"
              onClick={() => void submit()}
              disabled={busy}
              data-testid="role-manager-save"
            >
              {t("contacts.authz.manager.save")}
            </Button>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
