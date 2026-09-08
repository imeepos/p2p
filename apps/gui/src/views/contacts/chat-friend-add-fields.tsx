import { useTranslation } from "react-i18next";
import { PlusIcon, Trash2Icon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

import { fieldErrorAria, type FriendFieldError } from "./chat-friend-rules";

export function FieldError({ code, errorId }: { code?: FriendFieldError; errorId: string }) {
  const { t } = useTranslation();
  if (!code) return null;
  return (
    <p className="text-destructive text-xs" role="alert" id={errorId}>
      {t(`chat.addFriend.${code}`)}
    </p>
  );
}

export function NicknameField({
  value,
  onChange,
  error,
}: {
  value: string;
  onChange: (value: string) => void;
  error?: FriendFieldError;
}) {
  const { t } = useTranslation();
  const errorId = "friend-add-nickname-error";
  return (
    <div className="flex flex-col gap-1">
      <Label htmlFor="friend-add-nickname">{t("chat.addFriend.nicknameLabel")}</Label>
      <Input
        id="friend-add-nickname"
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={t("chat.addFriend.nicknamePlaceholder")}
        autoComplete="off"
        {...fieldErrorAria(errorId, error)}
      />
      <FieldError code={error} errorId={errorId} />
    </div>
  );
}

export function AddrRows({
  addrs,
  setAddrs,
  errors,
}: {
  addrs: string[];
  setAddrs: (update: (rows: string[]) => string[]) => void;
  errors?: Record<number, FriendFieldError>;
}) {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col gap-2">
      <Label>{t("chat.addFriend.addrsLabel")}</Label>
      {addrs.map((addr, index) => {
        const errorId = `friend-add-addr-error-${index}`;
        return (
          <div key={index} className="flex flex-col gap-1">
            <div className="flex items-center gap-2">
              <Input
                className="font-mono text-xs"
                value={addr}
                onChange={(event) =>
                  setAddrs((rows) =>
                    rows.map((row, i) => (i === index ? event.target.value : row)),
                  )
                }
                placeholder={t("chat.addFriend.addrPlaceholder")}
                aria-label={`${t("chat.addFriend.addrsLabel")} ${index + 1}`}
                autoComplete="off"
                {...fieldErrorAria(errorId, errors?.[index])}
              />
              <Button
                type="button"
                variant="ghost"
                size="icon"
                aria-label={t("chat.addFriend.removeAddr")}
                onClick={() => setAddrs((rows) => rows.filter((_, i) => i !== index))}
              >
                <Trash2Icon aria-hidden />
              </Button>
            </div>
            <FieldError code={errors?.[index]} errorId={errorId} />
          </div>
        );
      })}
      <Button
        type="button"
        variant="outline"
        size="sm"
        className="w-fit"
        onClick={() => setAddrs((rows) => [...rows, ""])}
      >
        <PlusIcon aria-hidden />
        {t("chat.addFriend.addAddr")}
      </Button>
    </div>
  );
}
