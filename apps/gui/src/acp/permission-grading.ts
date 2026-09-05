// 权限选项分级纯模型：视觉档位与确认门槛由 kind 唯一决定（P1 应答分级）。
// allow_once 单击即应答但降为次级样式防手滑；allow_always 持续放行风险最高，
// 必须经一次显式确认；reject 档用 danger 强调与 allow 档拉开色差。
import type { PermissionOption } from "./protocol";

export type PermissionActionKind = "allow-once" | "allow-always" | "reject";

export interface PermissionOptionGrade {
  action: PermissionActionKind;
  /** true 时单击只弹确认框，确认后才应答 */
  needsConfirm: boolean;
  variant: "secondary" | "outline";
  /** 附加强调色：allow_always 用 warning、reject 用 destructive；allow_once 无 */
  tone: "warning" | "danger" | null;
}

const GRADES: Record<PermissionActionKind, PermissionOptionGrade> = {
  "allow-once": { action: "allow-once", needsConfirm: false, variant: "secondary", tone: null },
  "allow-always": { action: "allow-always", needsConfirm: true, variant: "outline", tone: "warning" },
  reject: { action: "reject", needsConfirm: false, variant: "outline", tone: "danger" },
};

export function gradePermissionOption(
  option: Pick<PermissionOption, "kind">,
): PermissionOptionGrade {
  const kind = option.kind.toLowerCase();
  if (kind.startsWith("allow")) {
    return kind.includes("always") ? GRADES["allow-always"] : GRADES["allow-once"];
  }
  return GRADES.reject;
}
