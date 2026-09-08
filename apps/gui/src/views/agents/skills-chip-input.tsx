// skills chip 输入（拍板 Q9）：回车/逗号成 chip、Backspace 删末位、
// trim+去重+≤10（归一在 skills.ts 纯函数，组件只做交互与呈现）。
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { X } from "lucide-react";

import { Input } from "@/components/ui/input";

import { normalizeSkills, SKILLS_MAX } from "@/a2a/skills";

interface SkillsChipInputProps {
  value: string[];
  onChange: (skills: string[]) => void;
}

export function SkillsChipInput({ value, onChange }: SkillsChipInputProps) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState("");

  const commit = (): void => {
    const candidate = draft.trim();
    if (!candidate) return;
    const { skills } = normalizeSkills([...value, candidate]);
    onChange(skills);
    setDraft("");
  };

  return (
    <div className="flex flex-col gap-1">
      <div className="flex flex-wrap items-center gap-1" data-testid="agents-skills-chips">
        {value.map((skill) => (
          <span
            key={skill}
            className="bg-muted text-foreground inline-flex items-center gap-0.5 rounded px-1.5 py-0.5 text-xs"
          >
            {skill}
            <button
              type="button"
              aria-label={skill}
              data-testid={"agents-skill-remove-" + skill}
              onClick={() => onChange(value.filter((s) => s !== skill))}
              className="text-muted-foreground hover:text-foreground"
            >
              <X aria-hidden className="size-3" />
            </button>
          </span>
        ))}
        {value.length >= SKILLS_MAX ? (
          <span className="text-destructive text-xs">{t("agents.err.skillsMax")}</span>
        ) : null}
      </div>
      <Input
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === ",") {
            e.preventDefault();
            commit();
          }
          if (e.key === "Backspace" && draft === "" && value.length > 0) {
            onChange(value.slice(0, -1));
          }
        }}
        onBlur={commit}
        placeholder={t("agents.dialog.skillsPlaceholder")}
        autoComplete="off"
        data-testid="agents-skill-input"
      />
    </div>
  );
}
