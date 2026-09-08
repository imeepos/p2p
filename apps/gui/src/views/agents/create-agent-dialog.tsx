// 创建/编辑智能体对话框（endpoint-add-dialog 模式，设计 §8.2）：名称/描述/
// skills chip/可见性 SegmentedControl。编辑态（数据面 v1 只支持可见性变更，
// 契约 §17.2）名称/描述/技能禁用并显式说明；校验失败原位上浮，不静默。
import { useState } from "react";
import { useTranslation } from "react-i18next";

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
import { SegmentedControl } from "@/components/ui/segmented-control";
import { Textarea } from "@/components/ui/textarea";

import { normalizeSkills } from "@/a2a/skills";
import type { AgentCreateInput, AgentDefJson } from "@/a2a/types";

import { SkillsChipInput } from "./skills-chip-input";

interface CreateAgentDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** 非 null = 编辑态（仅可见性可改）。 */
  editing: AgentDefJson | null;
  onConfirm: (input: AgentCreateInput) => Promise<boolean>;
}

interface FieldErrors {
  name?: string;
  description?: string;
}

export function CreateAgentDialog({ open, onOpenChange, editing, onConfirm }: CreateAgentDialogProps) {
  const { t } = useTranslation();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [skills, setSkills] = useState<string[]>([]);
  const [visibility, setVisibility] = useState<"public" | "private">("public");
  const [errors, setErrors] = useState<FieldErrors | null>(null);
  const [submitting, setSubmitting] = useState(false);

  // 打开瞬间播种一次（渲染期状态调整，不落 effect）：编辑态回填原定义
  const [seededOpen, setSeededOpen] = useState(false);
  if (open !== seededOpen) {
    setSeededOpen(open);
    if (open) {
      setName(editing?.name ?? "");
      setDescription(editing?.description ?? "");
      setSkills(editing?.skills.map((s) => s.name) ?? []);
      setVisibility(editing ? (editing.visibility === "public" ? "public" : "private") : "public");
      setErrors(null);
    }
  }

  const validate = (): boolean => {
    const next: FieldErrors = {};
    if (!name.trim()) next.name = t("agents.err.nameRequired");
    if (!description.trim()) next.description = t("agents.err.descRequired");
    if (next.name || next.description) {
      setErrors(next);
      return false;
    }
    setErrors(null);
    return true;
  };

  const confirm = async (): Promise<void> => {
    if (!editing && !validate()) return;
    const { skills: normalized } = normalizeSkills(skills);
    setSubmitting(true);
    try {
      const ok = await onConfirm({
        name: name.trim(),
        description: description.trim(),
        skills: normalized,
        visibility,
      });
      if (ok) onOpenChange(false);
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md" data-testid="agents-create-dialog">
        <DialogHeader>
          <DialogTitle>
            {editing ? t("agents.dialog.editTitle") : t("agents.dialog.createTitle")}
          </DialogTitle>
          <DialogDescription>{t("agents.dialog.hint")}</DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-3">
          {editing ? (
            <p className="text-muted-foreground text-xs" data-testid="agents-edit-hint">
              {t("agents.dialog.editVisibilityOnly")}
            </p>
          ) : null}
          <div className="flex flex-col gap-1">
            <Label htmlFor="agents-name">{t("agents.dialog.nameLabel")}</Label>
            <Input
              id="agents-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t("agents.dialog.namePlaceholder")}
              disabled={!!editing}
              autoComplete="off"
              data-testid="agents-name-input"
            />
            {errors?.name ? (
              <span className="text-destructive text-xs" data-testid="agents-name-error">
                {errors.name}
              </span>
            ) : null}
          </div>
          <div className="flex flex-col gap-1">
            <Label htmlFor="agents-desc">{t("agents.dialog.descLabel")}</Label>
            <Textarea
              id="agents-desc"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder={t("agents.dialog.descPlaceholder")}
              disabled={!!editing}
              rows={2}
              data-testid="agents-desc-input"
            />
            {errors?.description ? (
              <span className="text-destructive text-xs" data-testid="agents-desc-error">
                {errors.description}
              </span>
            ) : null}
          </div>
          <div className="flex flex-col gap-1">
            <Label>{t("agents.dialog.skillsLabel")}</Label>
            <SkillsChipInput value={skills} onChange={setSkills} />
          </div>
          <div className="flex flex-col gap-1">
            <Label>{t("agents.dialog.visibilityLabel")}</Label>
            <SegmentedControl
              value={visibility}
              onChange={setVisibility}
              ariaLabel={t("agents.dialog.visibilityLabel")}
              options={[
                { value: "public", label: t("agents.dialog.visibilityPublic") },
                { value: "private", label: t("agents.dialog.visibilityPrivate") },
              ]}
            />
          </div>
        </div>
        <DialogFooter>
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            {t("common.actions.cancel")}
          </Button>
          <Button
            type="button"
            onClick={() => void confirm()}
            disabled={submitting}
            data-testid="agents-dialog-confirm"
          >
            {t("agents.dialog.confirm")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
