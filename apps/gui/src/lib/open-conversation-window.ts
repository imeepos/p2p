import { toastError } from "@/components/feedback/toast";
import i18n from "@/i18n";
import type { ConversationEntry } from "@/lib/conversation-entry";

// 独立窗口显示：Tauri 运行时开新 WebviewWindow 装载 /chat 选中态深链；
// 浏览器环境回退 window.open。失败路径 toast + console.error，禁静默。

const WINDOW_WIDTH = 960;
const WINDOW_HEIGHT = 680;

function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function conversationUrl(entry: ConversationEntry): string {
  const key = entry.kind === "friend" ? "peer" : entry.kind === "group" ? "group" : "agent";
  return "/chat?" + key + "=" + encodeURIComponent(entry.id);
}

function tauriWindowLabel(entry: ConversationEntry): string {
  const safe = entry.id.replace(/[^A-Za-z0-9_-]/g, "").slice(0, 48);
  return "chat-" + entry.kind + "-" + (safe || "conv");
}

async function openTauriWindow(entry: ConversationEntry): Promise<void> {
  const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  const label = tauriWindowLabel(entry);
  const existing = await WebviewWindow.getByLabel(label);
  if (existing) {
    await existing.setFocus();
    return;
  }
  const win = new WebviewWindow(label, {
    url: conversationUrl(entry),
    title: entry.title,
    width: WINDOW_WIDTH,
    height: WINDOW_HEIGHT,
  });
  await new Promise<void>((resolve, reject) => {
    win.once("tauri://created", () => resolve());
    win.once("tauri://error", (event) =>
      reject(new Error(String(event.payload ?? "webview window create failed"))),
    );
  });
}

export async function openConversationWindow(entry: ConversationEntry): Promise<void> {
  try {
    if (isTauriRuntime()) {
      await openTauriWindow(entry);
    } else {
      window.open(conversationUrl(entry), "_blank", "width=" + WINDOW_WIDTH + ",height=" + WINDOW_HEIGHT);
    }
  } catch (error) {
    console.error("[chat] 独立窗口打开失败", entry.kind, entry.id, error);
    toastError(i18n.t("chat.conversations.contextMenu.openWindowFailed"), {
      context: "chat.openConversationWindow",
    });
  }
}
