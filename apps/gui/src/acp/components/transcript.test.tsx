// 工具时间线与思考面板打磨测试（P2 页面打磨）：超长内容折叠、失败态红系
// 高亮、思考展开排版、自动滚底、错误结算徽章；直接播种 store 渲染
// Transcript，不依赖 mock WS 链路。
import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import { useAcpStore } from "@/acp/acp-store";
import type { TranscriptState, Turn } from "@/acp/transcript-model";
import { Transcript } from "./transcript";

await import("@/i18n");

const SID = "s-polish";

function seed(turns: Turn[]) {
  const state: TranscriptState = {
    turns,
    nextId: turns.length + 1,
    openAssistantId: null,
    openThoughtId: null,
    ignoredUpdates: 0,
  };
  useAcpStore.setState({ transcripts: { [SID]: state }, activeSessionId: SID });
}

function toolTurn(over: Partial<Extract<Turn, { kind: "tool" }>>): Turn {
  return {
    kind: "tool",
    id: 1,
    toolCallId: "call-1",
    title: "Reading config",
    toolKind: null,
    status: "completed",
    inputText: "",
    outputText: "",
    ...over,
  };
}

beforeEach(() => {
  useAcpStore.getState().resetConsoleState();
});

describe("ToolTurn 折叠与失败态", () => {
  it("超过约 6 行的入参/结果默认折叠，展开后全文可见且 aria-expanded 翻转", () => {
    const longOutput = Array.from({ length: 10 }, (_, i) => "line-" + i).join("\n");
    seed([toolTurn({ outputText: longOutput })]);
    render(<Transcript sessionId={SID} />);
    const body = screen.getByTestId("acp-tool-output-call-1");
    expect(body.textContent).not.toContain("line-9");
    expect(body.textContent).toContain("line-5");
    const toggle = screen.getByTestId("acp-tool-output-call-1-toggle");
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    expect(toggle.getAttribute("aria-controls")).toBe("acp-tool-output-call-1-body");
    fireEvent.click(toggle);
    expect(screen.getByTestId("acp-tool-output-call-1").textContent).toContain("line-9");
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    expect(toggle.textContent).toContain("收起");
  });

  it("短结果不渲染折叠开关，全文直出", () => {
    seed([toolTurn({ outputText: "ok" })]);
    render(<Transcript sessionId={SID} />);
    expect(screen.queryByTestId("acp-tool-output-call-1-toggle")).toBeNull();
    expect(screen.getByTestId("acp-tool-output-call-1").textContent).toContain("ok");
  });

  it("失败态工具节点红系整行高亮且状态徽章为失败", () => {
    seed([toolTurn({ status: "failed", toolKind: "execute", outputText: "exit 1" })]);
    render(<Transcript sessionId={SID} />);
    const node = screen.getByTestId("acp-turn-tool-call-1");
    expect(node.className).toContain("border-l-destructive");
    expect(node.className).toContain("bg-destructive/5");
    expect(screen.getByTestId("acp-tool-status-call-1").textContent).toContain("失败");
  });
});

describe("ThoughtTurn 展开排版", () => {
  it("展开后正文有行高与留白类，开关 aria-expanded 为 true", () => {
    seed([{ kind: "thought", id: 3, text: "step one", open: true }]);
    render(<Transcript sessionId={SID} />);
    const body = screen.getByTestId("acp-thought-body-3");
    expect(body.className).toContain("leading-6");
    expect(body.className).toContain("px-3");
    expect(screen.getByTestId("acp-thought-toggle-3").getAttribute("aria-expanded")).toBe("true");
  });
});

describe("Transcript 自动滚动", () => {
  function scrollEl(): HTMLElement {
    const el = screen.getByTestId("acp-transcript-scroll");
    Object.defineProperty(el, "scrollHeight", { configurable: true, value: 1000 });
    Object.defineProperty(el, "clientHeight", { configurable: true, value: 400 });
    return el;
  }

  function appendTurn(id: number) {
    act(() => {
      seed([
        ...useAcpStore.getState().transcripts[SID]!.turns,
        { kind: "assistant", id, text: "msg-" + id, streaming: false, stopReason: "end_turn" },
      ]);
    });
  }

  it("追加内容自动滚到底", () => {
    seed([{ kind: "user", id: 1, text: "hi" }]);
    render(<Transcript sessionId={SID} />);
    const el = scrollEl();
    appendTurn(2);
    expect(el.scrollTop).toBe(1000);
  });

  it("用户上滚查看历史时暂停跟随，滚回底部恢复跟随", () => {
    seed([{ kind: "user", id: 1, text: "hi" }]);
    render(<Transcript sessionId={SID} />);
    const el = scrollEl();
    appendTurn(2);
    expect(el.scrollTop).toBe(1000);
    // 手动上滚：远底（1000-10-400=590 >= 64）→ 暂停跟随
    el.scrollTop = 10;
    fireEvent.scroll(el);
    appendTurn(3);
    expect(el.scrollTop).toBe(10);
    // 滚回近底（1000-600-400=0 < 64）→ 恢复跟随
    el.scrollTop = 600;
    fireEvent.scroll(el);
    appendTurn(4);
    expect(el.scrollTop).toBe(1000);
  });
});

describe("AssistantTurn 错误结算徽章", () => {
  it("stopReason=error 渲染红色系失败徽章而非灰字", () => {
    seed([{ kind: "assistant", id: 9, text: "boom", streaming: false, stopReason: "error" }]);
    render(<Transcript sessionId={SID} />);
    const wrap = screen.getByTestId("acp-stop-reason-9");
    expect(wrap.textContent).toContain("失败");
    expect(wrap.querySelector("span")?.className).toContain("border-destructive/30");
  });

  it("普通 stopReason 仍为灰字，不受 error 徽章影响", () => {
    seed([{ kind: "assistant", id: 8, text: "ok", streaming: false, stopReason: "end_turn" }]);
    render(<Transcript sessionId={SID} />);
    const el = screen.getByTestId("acp-stop-reason-8");
    expect(el.textContent).toContain("正常结束");
    // testid 直落于灰字 span 本体，无 danger 徽章子元素
    expect(el.className).toContain("text-muted-foreground");
    expect(el.querySelector("span")).toBeNull();
  });
});
