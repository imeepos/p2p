import { useTranslation } from "react-i18next";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { useAuthzStore } from "@/stores/authz-store";

// 好友行/资料卡角色徽章（§18.4.2）：徽章仅展示角色名；过期灰显提示
// （判定面本就按 Expired 拒，展示层不重复渲染权限判定结果）。
// 未绑定好友不渲染徽章——「未绑定 = 全域拒绝」心智放角色对话框原话呈现。
export function FriendRoleBadge({ peerId }: { peerId: string }) {
  const { t } = useTranslation();
  // 过期判定取挂载时刻快照（react-hooks/purity：渲染期不调 Date.now，
  // discover-section nowSecs 同款）；列表重挂载即刷新。
  const [nowMs] = useState(() => Date.now());
  const binding = useAuthzStore((s) => s.bindings[peerId]);
  const role = useAuthzStore((s) =>
    s.roles.find((r) => r.roleId === binding?.roleId),
  );
  if (!binding) return null;
  const expired =
    binding.expiresAt !== undefined && binding.expiresAt * 1000 <= nowMs;
  return (
    <Badge
      variant={expired ? "outline" : "secondary"}
      className="shrink-0"
      data-testid={"contact-friend-role-" + peerId}
      title={expired ? t("contacts.authz.expiredBadge") : undefined}
    >
      {expired ? t("contacts.authz.expiredBadge") : (role?.name ?? binding.roleId)}
    </Badge>
  );
}

// 资料卡角色行的未绑定占位（有绑定时不渲染，徽章即内容）。
export function RoleUnboundHint({ peerId }: { peerId: string }) {
  const { t } = useTranslation();
  const bound = useAuthzStore((s) => s.bindings[peerId] != null);
  if (bound) return null;
  return (
    <span className="text-muted-foreground text-sm" data-testid={"contact-friend-role-none-" + peerId}>
      {t("contacts.authz.unbound")}
    </span>
  );
}
