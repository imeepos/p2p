import { useTranslation } from "react-i18next";

import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { permLabel } from "@/views/contacts/role-perm-labels";

interface RoleEditorFieldsProps {
  isCreate: boolean;
  roleId: string;
  onRoleIdChange: (value: string) => void;
  name: string;
  onNameChange: (value: string) => void;
  note: string;
  onNoteChange: (value: string) => void;
  /** §4 闭集（store.permissions），角色管理复选框数据源 */
  permissions: string[];
  selected: string[];
  onToggle: (permKey: string) => void;
}

// 角色表单字段组（create/edit 共用）：roleId 仅新建可填；权限复用原生
// input[type=checkbox]（share-create-form 先例，ui 库无 checkbox 基础件）。
export function RoleEditorFields(props: RoleEditorFieldsProps) {
  const { t } = useTranslation();
  return (
    <>
      <div className="flex flex-col gap-1">
        <Label htmlFor="role-manager-role-id">{t("contacts.authz.manager.roleIdLabel")}</Label>
        <Input
          id="role-manager-role-id"
          value={props.roleId}
          onChange={(event) => props.onRoleIdChange(event.target.value)}
          placeholder={t("contacts.authz.manager.roleIdPlaceholder")}
          disabled={!props.isCreate}
          autoComplete="off"
          data-testid="role-manager-role-id"
        />
      </div>
      <div className="flex flex-col gap-1">
        <Label htmlFor="role-manager-name">{t("contacts.authz.manager.nameLabel")}</Label>
        <Input
          id="role-manager-name"
          value={props.name}
          onChange={(event) => props.onNameChange(event.target.value)}
          placeholder={t("contacts.authz.manager.namePlaceholder")}
          autoComplete="off"
          data-testid="role-manager-name"
        />
      </div>
      <div className="flex flex-col gap-1">
        <Label htmlFor="role-manager-note">{t("contacts.authz.manager.noteLabel")}</Label>
        <Textarea
          id="role-manager-note"
          rows={2}
          value={props.note}
          onChange={(event) => props.onNoteChange(event.target.value)}
          data-testid="role-manager-note"
        />
      </div>
      <div className="flex flex-col gap-1">
        <span className="text-sm font-medium">
          {t("contacts.authz.manager.permissionsLabel")}
          <span className="text-muted-foreground ml-1.5 text-xs font-normal">
            {t("contacts.authz.manager.permissionsHint")}
          </span>
        </span>
        <div className="flex flex-col gap-1.5" data-testid="role-manager-permissions">
          {props.permissions.map((permKey) => (
            <label
              key={permKey}
              className="flex cursor-pointer items-center gap-1.5 text-sm"
              data-testid={"role-perm-label-" + permKey}
            >
              <input
                type="checkbox"
                checked={props.selected.includes(permKey)}
                onChange={() => props.onToggle(permKey)}
                data-testid={"role-perm-" + permKey}
              />
              {permLabel(permKey, t)}
            </label>
          ))}
        </div>
      </div>
    </>
  );
}
