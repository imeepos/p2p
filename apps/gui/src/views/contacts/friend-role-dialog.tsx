import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { SettingsIcon, UnlinkIcon } from "lucide-react";

import { CommandErrorText } from "@/components/feedback/command-error";
import { toastError, toastSuccess } from "@/components/feedback/toast";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { ChatFriendJson } from "@/lib/ipc-types";
import { useAuthzStore } from "@/stores/authz-store";
import { RoleManagerDialog } from "@/views/contacts/role-manager-dialog";
import { errorText } from "@/views/shared/form-flow";

const NONE_SENTINEL = "__none__";
const DAY_SECS = 86_400;

interface FriendRoleDialogProps {
  friend: ChatFriendJson;
  onOpenChange: (open: boolean) => void;
}

// 好友角色编辑对话框（§18.1/§18.4）：角色下拉（内建四 + 自定义）+ 可选过期
// （常用时长快捷换算，允许直填 Unix 秒）+ 解绑；底部附加好友默认角色设置
// （authz_default_role_save，空串 = 不自动绑）。绑定/解绑成功本地推进并关闭；
// 校验失败与命令失败原位/可读呈现（三态：绑定成功/解绑/校验失败）。
export function FriendRoleDialog({ friend, onOpenChange }: FriendRoleDialogProps) {
  const { t } = useTranslation();
  const roles = useAuthzStore((s) => s.roles);
  const loadError = useAuthzStore((s) => s.loadError);
  const loadAll = useAuthzStore((s) => s.loadAll);
  const bindRole = useAuthzStore((s) => s.bindRole);
  const unbindRole = useAuthzStore((s) => s.unbindRole);
  const defaultRoleId = useAuthzStore((s) => s.defaultRoleId);
  const saveDefaultRole = useAuthzStore((s) => s.saveDefaultRole);
  const binding = useAuthzStore((s) => s.bindings[friend.peerId]);

  const [roleId, setRoleId] = useState(binding?.roleId ?? "");
  const [expiryChoice, setExpiryChoice] = useState<"never" | "1" | "7" | "30" | "custom">(
    "never",
  );
  const [customExpiry, setCustomExpiry] = useState(
    binding?.expiresAt ? String(binding.expiresAt) : "",
  );
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [managing, setManaging] = useState(false);

  useEffect(() => {
    if (roles.length === 0) void loadAll();
  }, [roles.length, loadAll]);

  const resolveExpiresAt = (): number | null | "invalid" => {
    if (expiryChoice === "never") return null;
    if (expiryChoice !== "custom") {
      return Math.floor(Date.now() / 1000) + Number(expiryChoice) * DAY_SECS;
    }
    const parsed = Number(customExpiry);
    return Number.isInteger(parsed) && parsed > 0 ? parsed : "invalid";
  };

  const submit = async () => {
    if (!roleId) {
      setError(t("contacts.authz.roleRequired"));
      return;
    }
    const expiresAt = resolveExpiresAt();
    if (expiresAt === "invalid") {
      setError(t("contacts.authz.expiryInvalid"));
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      await bindRole(friend.peerId, roleId, expiresAt);
      toastSuccess(t("contacts.authz.bindSuccess"));
      onOpenChange(false);
    } catch (err) {
      console.error("[contacts] 角色绑定失败", err);
      const detail = errorText(err);
      setError(detail);
      toastError(t("contacts.authz.bindFailed"), { description: detail, context: "authz_bind" });
    } finally {
      setSubmitting(false);
    }
  };

  const unbind = async () => {
    setSubmitting(true);
    setError(null);
    try {
      await unbindRole(friend.peerId);
      toastSuccess(t("contacts.authz.unbindSuccess"));
      onOpenChange(false);
    } catch (err) {
      console.error("[contacts] 角色解绑失败", err);
      const detail = errorText(err);
      setError(detail);
      toastError(t("contacts.authz.unbindFailed"), { description: detail, context: "authz_unbind" });
    } finally {
      setSubmitting(false);
    }
  };

  const changeDefaultRole = async (value: string) => {
    try {
      await saveDefaultRole(value === NONE_SENTINEL ? "" : value);
      toastSuccess(t("contacts.authz.defaultRoleSaveSuccess"));
    } catch (err) {
      console.error("[contacts] 默认角色保存失败", err);
      toastError(t("contacts.authz.defaultRoleSaveFailed"), {
        description: errorText(err),
        context: "authz_default_role_save",
      });
    }
  };

  return (
    <>
      <Dialog open onOpenChange={onOpenChange}>
        <DialogContent className="sm:max-w-md" data-testid="friend-role-dialog">
          <DialogHeader>
            <DialogTitle>{t("contacts.authz.dialogTitle")}</DialogTitle>
            <DialogDescription>{t("contacts.authz.dialogDescription")}</DialogDescription>
          </DialogHeader>
          <div className="flex flex-col gap-3">
            <div className="flex flex-col gap-1">
              <Label htmlFor="friend-role-select">{t("contacts.authz.roleLabel")}</Label>
              <Select
                value={roleId}
                onValueChange={(value) => {
                  setRoleId(value);
                  if (error) setError(null);
                }}
              >
                <SelectTrigger
                  id="friend-role-select"
                  className="w-full"
                  data-testid="friend-role-select"
                >
                  <SelectValue placeholder={t("contacts.authz.rolePlaceholder")} />
                </SelectTrigger>
                <SelectContent>
                  {roles.map((role) => (
                    <SelectItem key={role.roleId} value={role.roleId}>
                      {role.name}
                      <span className="text-muted-foreground ml-1.5 text-xs">
                        {role.builtin
                          ? t("contacts.authz.builtinTag")
                          : t("contacts.authz.customTag")}
                      </span>
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="self-start"
                onClick={() => setManaging(true)}
                data-testid="friend-role-manage"
              >
                <SettingsIcon aria-hidden className="size-4" />
                {t("contacts.authz.manager.open")}
              </Button>
            </div>
            <div className="flex flex-col gap-1">
              <Label htmlFor="friend-role-expiry">{t("contacts.authz.expiryLabel")}</Label>
              <Select
                value={expiryChoice}
                onValueChange={(value) => setExpiryChoice(value as typeof expiryChoice)}
              >
                <SelectTrigger id="friend-role-expiry" className="w-full" data-testid="friend-role-expiry">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="never">{t("contacts.authz.expiryNever")}</SelectItem>
                  <SelectItem value="1">{t("contacts.authz.expiryIn1d")}</SelectItem>
                  <SelectItem value="7">{t("contacts.authz.expiryIn7d")}</SelectItem>
                  <SelectItem value="30">{t("contacts.authz.expiryIn30d")}</SelectItem>
                  <SelectItem value="custom">{t("contacts.authz.expiryCustom")}</SelectItem>
                </SelectContent>
              </Select>
              {expiryChoice === "custom" ? (
                <Input
                  value={customExpiry}
                  onChange={(event) => setCustomExpiry(event.target.value)}
                  placeholder={t("contacts.authz.expiryCustomLabel")}
                  inputMode="numeric"
                  autoComplete="off"
                  data-testid="friend-role-expiry-custom"
                />
              ) : null}
            </div>
            <div className="flex flex-col gap-1">
              <Label htmlFor="friend-role-default">{t("contacts.authz.defaultRoleLabel")}</Label>
              <Select
                value={defaultRoleId === "" ? NONE_SENTINEL : defaultRoleId}
                onValueChange={(value) => void changeDefaultRole(value)}
              >
                <SelectTrigger id="friend-role-default" className="w-full" data-testid="friend-role-default">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value={NONE_SENTINEL}>{t("contacts.authz.defaultRoleNone")}</SelectItem>
                  {roles.map((role) => (
                    <SelectItem key={role.roleId} value={role.roleId}>
                      {role.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <p className="text-muted-foreground text-xs">{t("contacts.authz.defaultRoleHint")}</p>
            </div>
            {!binding ? (
              <p className="text-muted-foreground text-xs" data-testid="friend-role-unbound-hint">
                {t("contacts.authz.unboundHint")}
              </p>
            ) : null}
            {loadError ? (
              <p className="text-destructive text-xs" data-testid="friend-role-load-error">
                {t("contacts.authz.loadFailed")}
              </p>
            ) : null}
            {error ? (
              <CommandErrorText
                message={error}
                prefix={t("contacts.authz.bindFailed")}
                testId="friend-role-error"
              />
            ) : null}
          </div>
          <DialogFooter className="sm:justify-between">
            {binding ? (
              <Button
                type="button"
                variant="ghost"
                className="text-destructive hover:text-destructive"
                onClick={() => void unbind()}
                disabled={submitting}
                data-testid="friend-role-unbind"
              >
                <UnlinkIcon aria-hidden className="size-4" />
                {t("contacts.authz.unbind")}
              </Button>
            ) : null}
            <div className="flex gap-2">
              <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
                {t("common.actions.cancel")}
              </Button>
              <Button
                type="button"
                onClick={() => void submit()}
                disabled={submitting}
                data-testid="friend-role-submit"
              >
                {binding ? t("contacts.authz.rebind") : t("contacts.authz.bind")}
              </Button>
            </div>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      {managing ? <RoleManagerDialog onOpenChange={setManaging} /> : null}
    </>
  );
}
