import { toastError } from "@/components/feedback/toast";
import i18n from "@/i18n";
import { ipc } from "@/lib/ipc";

export const MAX_NOTES_CHARS = 400;

// 发布说明超长截断展示；完整内容经 update_open_release_page 跳浏览器。
export function truncateNotes(notes: string): string {
  if (notes.length <= MAX_NOTES_CHARS) return notes;
  return notes.slice(0, MAX_NOTES_CHARS) + "…";
}

// 下载与完整说明统一走白名单命令；失败 toast 可见，禁止静默吞。
export async function openReleasePage(url: string | null): Promise<void> {
  if (!url) return;
  try {
    await ipc.updateOpenReleasePage(url);
  } catch (error) {
    console.error("[update] update_open_release_page 失败", error);
    toastError(i18n.t("update.errors.openPageFailed"), {
      description: error instanceof Error ? error.message : String(error),
    });
  }
}
