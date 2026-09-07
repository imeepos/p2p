import { useTranslation } from "react-i18next";
import { UsersRoundIcon } from "lucide-react";

import type { ContactEntity } from "./contacts-detail-model";
import { DetailAgent } from "./detail-agent";
import { DetailFriend } from "./detail-friend";
import { DetailGroup } from "./detail-group";
import { DetailInvite } from "./detail-invite";

// 右栏资料卡分派（双栏改版）：按选中实体渲染对应资料卡；无选中且无
// 可回退实体时空态引导。
export function ContactsDetailPane({ entity }: { entity: ContactEntity | null }) {
  const { t } = useTranslation();
  if (entity?.friend) return <DetailFriend friend={entity.friend} />;
  if (entity?.group) return <DetailGroup group={entity.group} />;
  if (entity?.agent) return <DetailAgent agent={entity.agent} />;
  if (entity?.invite) return <DetailInvite invite={entity.invite} />;
  return (
    <div className="flex h-full flex-col items-center justify-center gap-2 p-8 text-center">
      <UsersRoundIcon aria-hidden className="text-muted-foreground/60 size-10" />
      <p className="text-sm font-medium">{t("contacts.detail.emptyTitle")}</p>
      <p className="text-muted-foreground max-w-60 text-xs">{t("contacts.detail.emptyHint")}</p>
    </div>
  );
}
