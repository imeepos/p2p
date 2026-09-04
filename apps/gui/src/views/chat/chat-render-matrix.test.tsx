// IM-T47 渲染矩阵：kind ∈ {text,image,audio,video,file} × {me 发送成功, me 发送失败, them 入站}
// 共 15 格 + 补缺口（列表摘要/混合排序/未知 kind 防御/状态角标共存）。只加测试不改生产。
import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ChatFriendJson, ChatKind, ChatMessageJson, ChatSendReport, NodeEventHandler } from "@/lib/ipc-types";
import { chatMedia, friendJson, mediaMessage, peerId, sendReport, textMessage } from "@/test/chat-boundaries-fixtures";
import {
  MATRIX_PEER,
  bubbleArea,
  conversationRow,
  deferred,
  isBefore,
  mediaFile,
  mountChat,
  resetChatStore,
  seedConversation,
  seedSummaries,
} from "@/test/chat-render-matrix-fixtures";

const { mocks, toastSpies } = vi.hoisted(() => ({
  mocks: {
    friends: vi.fn<() => Promise<ChatFriendJson[]>>(),
    history: vi.fn<(peer: string, beforeId?: string | null, limit?: number) => Promise<ChatMessageJson[]>>(),
    send: vi.fn<() => Promise<ChatSendReport>>(),
    // 群/1:1 两个 store 各注册一个监听（真实 ipc 事件总线一对多）
    handlers: [] as NodeEventHandler[],
  },
  toastSpies: { error: vi.fn() },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatFriendsList: mocks.friends,
    chatHistory: mocks.history,
    chatSend: mocks.send,
    onNodeEvent: (handler: NodeEventHandler) => {
      mocks.handlers.push(handler);
      return Promise.resolve(() => {});
    },
  },
}));

// 穿透真实 toastError：sonner 照常渲染（可断言错误 toast 可见），spy 记录每次调用供失败格断言。
vi.mock("@/components/feedback/toast", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/components/feedback/toast")>();
  return {
    ...actual,
    toastError: (...args: Parameters<typeof actual.toastError>) => {
      toastSpies.error(...args);
      return actual.toastError(...args);
    },
  };
});

import "@/i18n";

// 一对多投递（群/1:1 store 各自订阅），act 包裹与 makeEmitter 同纪律。
const emit = (event: Parameters<NodeEventHandler>[0]): void => {
  act(() => {
    for (const handler of mocks.handlers) handler(event);
  });
};
const PEER = MATRIX_PEER;

interface MediaRow {
  kind: ChatKind;
  fileName: string;
  mime: string;
  path: string;
  tag: "img" | "audio" | "video" | null;
}

// 四类媒体矩阵行：path 可内联（asset:// 命中 media-content.inlineSrc），file 走裸路径信息卡。
const MEDIA_ROWS: MediaRow[] = [
  { kind: "image", fileName: "photo.png", mime: "image/png", path: "asset://chat/photo.png", tag: "img" },
  { kind: "audio", fileName: "voice.mp3", mime: "audio/mpeg", path: "asset://chat/voice.mp3", tag: "audio" },
  { kind: "video", fileName: "clip.mp4", mime: "video/mp4", path: "asset://chat/clip.mp4", tag: "video" },
  { kind: "file", fileName: "archive.zip", mime: "application/zip", path: "/data/files/archive.zip", tag: null },
];

beforeEach(() => {
  vi.clearAllMocks();
  resetChatStore();
  mocks.friends.mockResolvedValue([]);
  mocks.history.mockResolvedValue([]);
});

describe("渲染矩阵·me 发送成功", () => {
  it("text：占位上屏→回执替换→气泡文本换行保留", async () => {
    const text = "第一行\n第二行";
    const gate = deferred<ChatSendReport>();
    mocks.send.mockImplementation(() => gate.promise);
    seedConversation([]);
    mountChat();
    await screen.findByTestId("chat-input");
    fireEvent.change(screen.getByTestId("chat-input"), { target: { value: text } });
    fireEvent.click(screen.getByTestId("chat-send"));
    expect(mocks.send).toHaveBeenCalledWith(PEER, "text", text);
    expect(within(bubbleArea()).getByTestId("message-status").textContent).toBe("等待对方上线");
    await act(async () => gate.resolve(sendReport(textMessage("sx1", PEER, text, { status: "delivered" }))));
    const p = bubbleArea().querySelector("p.whitespace-pre-wrap");
    expect(p?.textContent).toBe(text); // textContent 原始换行保留，未截断未重排
    expect(p?.className).toContain("whitespace-pre-wrap");
    expect(within(bubbleArea()).getByTestId("message-status").textContent).toBe("已送达");
  });

  it.each(MEDIA_ROWS)("$kind：占位上屏→回执替换→按类型渲染", async (row) => {
    const gate = deferred<ChatSendReport>();
    mocks.send.mockImplementation(() => gate.promise);
    seedConversation([]);
    mountChat();
    await screen.findByTestId("chat-input");
    fireEvent.change(screen.getByTestId("chat-file-input"), {
      target: { files: [mediaFile(row.fileName, row.mime)] },
    });
    // 占位上屏：类型内容（文件名）与发送中角标、取消按钮共存
    expect(await within(bubbleArea()).findByText(row.fileName)).toBeTruthy();
    expect(within(bubbleArea()).getByTestId("message-status").textContent).toBe("等待对方上线");
    expect(screen.getByRole("button", { name: "取消发送" })).toBeTruthy();
    const real = mediaMessage(`ok-${row.kind}`, PEER, row.kind, chatMedia(row.fileName, row.mime, 1, row.path), { status: "delivered", tsMs: 9000 });
    await act(async () => gate.resolve(sendReport(real)));
    if (row.tag) {
      await waitFor(() => expect(bubbleArea().querySelector(row.tag!)).toBeTruthy());
      const el = bubbleArea().querySelector(row.tag!) as HTMLMediaElement;
      expect(el.getAttribute("src")).toBe(row.path);
      if (row.tag !== "img") expect(el.hasAttribute("controls")).toBe(true);
    } else {
      expect(within(bubbleArea()).getByRole("link", { name: "下载" }).getAttribute("href")).toBe(row.path);
      const sizeDiv = within(bubbleArea()).getByText(row.fileName).parentElement?.querySelector("div.text-xs");
      expect((sizeDiv?.textContent ?? "").length).toBeGreaterThan(0); // 大小可读展示
      expect(bubbleArea().querySelector("img,audio,video")).toBeNull();
    }
    expect(within(bubbleArea()).getByTestId("message-status").textContent).toBe("已送达");
    expect(screen.queryByRole("button", { name: "取消发送" })).toBeNull();
  });
});

describe("渲染矩阵·me 发送失败", () => {
  it("text：占位回滚移除、错误 toast 实渲染可见、界面不白屏", async () => {
    const gate = deferred<ChatSendReport>();
    mocks.send.mockImplementation(() => gate.promise);
    seedConversation([]);
    mountChat();
    await screen.findByTestId("chat-input");
    fireEvent.change(screen.getByTestId("chat-input"), { target: { value: "失败文本" } });
    fireEvent.click(screen.getByTestId("chat-send"));
    await act(async () => gate.reject(new Error("离线发送失败")));
    await waitFor(() =>
      expect(toastSpies.error).toHaveBeenCalledWith("发送失败", expect.objectContaining({ description: "离线发送失败" })));
    await screen.findByText("发送失败"); // sonner 实渲染（本格是文件内首个 toastError，未被去重拦截）
    expect(within(bubbleArea()).queryByText("失败文本")).toBeNull();
    expect(screen.getByTestId("chat-input")).toBeTruthy();
    expect(screen.getByRole("region", { name: "会话" })).toBeTruthy();
  });

  it.each(MEDIA_ROWS)("$kind：占位回滚移除、错误可见、界面不白屏", async (row) => {
    const gate = deferred<ChatSendReport>();
    mocks.send.mockImplementation(() => gate.promise);
    seedConversation([]);
    mountChat();
    await screen.findByTestId("chat-input");
    fireEvent.change(screen.getByTestId("chat-file-input"), {
      target: { files: [mediaFile(row.fileName, row.mime)] },
    });
    expect(await within(bubbleArea()).findByText(row.fileName)).toBeTruthy();
    await act(async () => gate.reject(new Error("离线发送失败")));
    await waitFor(() =>
      expect(toastSpies.error).toHaveBeenCalledWith("发送失败", expect.objectContaining({ description: "离线发送失败" })));
    expect(within(bubbleArea()).queryByText(row.fileName)).toBeNull();
    expect(screen.getByTestId("chat-input")).toBeTruthy();
    expect(screen.getByTestId("chat-file-input")).toBeTruthy();
  });
});

describe("渲染矩阵·them 入站渲染", () => {
  it("text：chat_message 注入→气泡渲染且换行保留，无状态角标", async () => {
    seedConversation([]);
    mountChat();
    await screen.findByTestId("chat-input");
    emit({ type: "chat_message", peer: PEER, message: textMessage("in-t1", PEER, "您好\n请查收", { sender: "them" }) });
    const p = bubbleArea().querySelector("p.whitespace-pre-wrap");
    expect(p?.textContent).toBe("您好\n请查收"); // 换行按原文保留
    expect(p?.className).toContain("whitespace-pre-wrap");
    expect(within(bubbleArea()).queryByTestId("message-status")).toBeNull();
  });

  it.each(MEDIA_ROWS)("$kind：chat_message 注入→按类型渲染", async (row) => {
    seedConversation([]);
    mountChat();
    await screen.findByTestId("chat-input");
    emit({
      type: "chat_message",
      peer: PEER,
      message: mediaMessage(`in-${row.kind}`, PEER, row.kind, chatMedia(row.fileName, row.mime, 2048, row.path), { sender: "them", status: "delivered" }),
    });
    if (row.tag) {
      await waitFor(() => expect(bubbleArea().querySelector(row.tag!)).toBeTruthy());
      const el = bubbleArea().querySelector(row.tag!) as HTMLMediaElement;
      expect(el.getAttribute("src")).toBe(row.path);
      if (row.tag !== "img") expect(el.hasAttribute("controls")).toBe(true);
    } else {
      expect(await within(bubbleArea()).findByText(row.fileName)).toBeTruthy();
      expect(within(bubbleArea()).getByRole("link", { name: "下载" }).getAttribute("href")).toBe(row.path);
      expect(bubbleArea().querySelector("img,audio,video")).toBeNull();
    }
  });

  it("text 2000 字符恰好完整渲染，无异常无截断", async () => {
    seedConversation([]);
    mountChat();
    await screen.findByTestId("chat-input");
    const text = "A".repeat(2000);
    emit({ type: "chat_message", peer: PEER, message: textMessage("in-big", PEER, text, { sender: "them" }) });
    await waitFor(() => expect(within(bubbleArea()).getByText(text)).toBeTruthy());
    expect(within(bubbleArea()).getByText(text).closest("p")?.textContent?.length).toBe(2000);
  });
});

describe("渲染矩阵·补缺口", () => {
  it("会话列表最后消息摘要按类型显示可读文件名，非二进制或空串", () => {
    const entries = (
      [
        ["image", "photo.png", "image/png"],
        ["audio", "voice.mp3", "audio/mpeg"],
        ["video", "movie.mp4", "video/mp4"],
        ["file", "report.pdf", "application/pdf"],
      ] as const
    ).map(([kind, name, mime], index) => ({
      peer: peerId(`sum-${kind}`),
      message: mediaMessage(`s${index}`, PEER, kind, chatMedia(name, mime, 1, null), { tsMs: 5000 - index }),
    }));
    entries.push({ peer: peerId("sum-text"), message: textMessage("s9", PEER, "纯文本摘要", { tsMs: 100 }) });
    seedSummaries(entries);
    mountChat();
    expect(conversationRow(entries[0].peer).textContent).toContain("photo.png");
    expect(conversationRow(entries[1].peer).textContent).toContain("voice.mp3");
    expect(conversationRow(entries[2].peer).textContent).toContain("movie.mp4");
    expect(conversationRow(entries[3].peer).textContent).toContain("report.pdf");
    expect(conversationRow(entries[4].peer).textContent).toContain("纯文本摘要");
    expect((conversationRow(entries[0].peer).textContent ?? "").match(/base64|data:/)).toBeNull();
  });

  it("混合类型序列按时间升序渲染，乱序到达后顺序稳定", async () => {
    const oldest = textMessage("mix-t", PEER, "最早文本", { tsMs: 1000, sender: "them" });
    const mid = mediaMessage("mix-i", PEER, "image", chatMedia("mid.png", "image/png", 1, "asset://chat/mid.png"), { tsMs: 2000, sender: "them", status: "delivered" });
    const newest = mediaMessage("mix-v", PEER, "video", chatMedia("last.mp4", "video/mp4", 1, "asset://chat/last.mp4"), { tsMs: 3000, sender: "them", status: "delivered" });
    // 乱序页走真实 selectPeer → mergeMessages 排序路径
    mocks.friends.mockResolvedValue([friendJson(PEER, "矩阵好友")]);
    mocks.history.mockImplementation(async (peer: string) => (peer === PEER ? [newest, oldest, mid] : []));
    mountChat();
    fireEvent.click(await screen.findByText("矩阵好友"));
    await waitFor(() => expect(bubbleArea().querySelector("video")).toBeTruthy());
    const area = bubbleArea();
    const oldestP = within(area).getByText("最早文本");
    const img = area.querySelector("img")!;
    const video = area.querySelector("video")!;
    expect(isBefore(oldestP, img)).toBe(true);
    expect(isBefore(img, video)).toBe(true);
    emit({ type: "chat_message", peer: PEER, message: textMessage("mix-x", PEER, "插入其间", { tsMs: 2500, sender: "them" }) });
    const inserted = within(area).getByText("插入其间");
    expect(isBefore(img, inserted)).toBe(true);
    expect(isBefore(inserted, video)).toBe(true);
    expect(isBefore(oldestP, img)).toBe(true);
  });

  it("未知 kind：不白屏、降级渲染气泡壳；带媒体时信息卡兜底", async () => {
    seedConversation([]);
    mountChat();
    await screen.findByTestId("chat-input");
    const unknownBase = { peer: PEER, sender: "them" as const, kind: "sticker" as ChatKind, text: null, status: "delivered" as const };
    emit({ type: "chat_message", peer: PEER, message: { id: "unk1", tsMs: 1000, media: null, ...unknownBase } });
    await waitFor(() => expect(bubbleArea().querySelectorAll("time").length).toBe(1)); // 气泡壳降级存在
    expect(screen.getByTestId("chat-input")).toBeTruthy();
    emit({
      type: "chat_message",
      peer: PEER,
      message: { id: "unk2", tsMs: 2000, media: chatMedia("hologram.card", "application/x-sticker", 7, null), ...unknownBase },
    });
    expect(await within(bubbleArea()).findByText("hologram.card")).toBeTruthy(); // 信息卡降级展示
    expect(bubbleArea().querySelectorAll("time").length).toBe(2);
  });

  it("媒体消息状态角标（pending/failed）与类型内容共存显示", () => {
    const pendingImg = mediaMessage("bd-p", PEER, "image", chatMedia("pending.png", "image/png", 1, null), { status: "pending" });
    const failedZip = mediaMessage("bd-f", PEER, "file", chatMedia("failed.zip", "application/zip", 2, "/data/failed.zip"), { status: "failed", tsMs: 2000 });
    seedConversation([failedZip, pendingImg]);
    mountChat();
    const area = bubbleArea();
    expect(within(area).getByText("pending.png")).toBeTruthy();
    expect(within(area).getByText("failed.zip")).toBeTruthy();
    expect(area.querySelectorAll('[data-testid="message-status"]').length).toBe(2);
    expect(within(area).getByText("等待对方上线")).toBeTruthy();
    expect(within(area).getByText("失败")).toBeTruthy();
    expect(screen.getByRole("button", { name: "取消发送" })).toBeTruthy();
  });

  it("media：img/video 标签限宽 max-w-full 防溢出气泡", () => {
    seedConversation([
      mediaMessage("mw-i", PEER, "image", chatMedia("wide.png", "image/png", 1, "asset://chat/wide.png")),
      mediaMessage("mw-v", PEER, "video", chatMedia("wide.mp4", "video/mp4", 2, "asset://chat/wide.mp4"), { tsMs: 2000 }),
    ]);
    mountChat();
    const area = bubbleArea();
    expect(area.querySelector("img")?.className).toContain("max-w-full");
    expect(area.querySelector("video")?.className).toContain("max-w-full");
  });

  it("failed 角标 AA 色：me 气泡内用双主题红类而非 text-destructive", () => {
    seedConversation([
      mediaMessage("cf-1", PEER, "file", chatMedia("bad.zip", "application/zip", 1, "/data/bad.zip"), { status: "failed" }),
    ]);
    mountChat();
    const badge = bubbleArea().querySelector('[data-testid="message-status"]');
    expect(badge?.className).toContain("text-red-300");
    expect(badge?.className).toContain("dark:text-red-700");
    expect(badge?.className).not.toContain("text-destructive");
  });
});