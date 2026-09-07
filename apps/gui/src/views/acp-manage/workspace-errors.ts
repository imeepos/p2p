// WorkspaceAdminError → i18n 键映射（词法码与 agent api.rs 一一对应）。
import type { I18nKey } from "@/i18n/types";
import { WorkspaceAdminError } from "@/acp/share-admin-client";

const CODE_KEYS: Record<string, I18nKey> = {
  "invalid-field": "acpManage.workspaces.invalidField",
  "duplicate-id": "acpManage.workspaces.duplicate",
  "invalid-dir": "acpManage.workspaces.dirInvalid",
  "unknown-workspace": "acpManage.workspaces.unknown",
  "legacy-default": "acpManage.workspaces.legacy",
  store: "acpManage.workspaces.store",
};

export function workspaceErrorKey(error: unknown): I18nKey {
  if (error instanceof WorkspaceAdminError) {
    return CODE_KEYS[error.code] ?? "acpManage.workspaces.store";
  }
  return "acpManage.workspaces.loadFailed";
}
