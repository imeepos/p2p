// skills chip 输入纯函数（设计拍板 Q9）：≤10 条、trim、去重；
// chip 字符串 → AgentSkill 双写（id=slug 同宿主 [a-z0-9-] 字符集，name=原文）。
import type { AgentSkillJson } from "./types";

export const SKILLS_MAX = 10;

/** 归一一批 chip 输入：trim + 去空 + 去重 + 截断上限。 */
export function normalizeSkills(raw: string[]): { skills: string[]; dropped: number } {
  const seen = new Set<string>();
  const skills: string[] = [];
  for (const item of raw) {
    const trimmed = item.trim();
    if (!trimmed || seen.has(trimmed)) continue;
    if (skills.length >= SKILLS_MAX) {
      return { skills, dropped: raw.length - skills.length };
    }
    seen.add(trimmed);
    skills.push(trimmed);
  }
  return { skills, dropped: 0 };
}

/** djb2 稳定 hash（hex）：非 ASCII chip 的 id 兜底，同输入同 id。 */
function stableHash(input: string): string {
  let hash = 5381;
  for (let i = 0; i < input.length; i += 1) {
    hash = ((hash << 5) + hash + input.charCodeAt(i)) >>> 0;
  }
  return hash.toString(16);
}

/** chip 文本 → AgentSkill：id 仅 [a-z0-9-]（宿主校验同口径）；
 *  非 ASCII（中文等）slug 化为空时用 skill-<djb2> 兜底，绝不静默丢卡。 */
export function toSkillJson(skill: string): AgentSkillJson | null {
  const name = skill.trim();
  if (!name) return null;
  const slug = name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 32);
  const id = slug || ("skill-" + stableHash(name));
  return { id, name };
}
