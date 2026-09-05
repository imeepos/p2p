// 工具时间线与思考面板打磨测试（P2 页面打磨）：超长内容折叠、失败态红系
// 高亮、思考展开排版；直接播种 store 渲染 Transcript，不依赖 mock WS 链路。
import { fireEvent, render, screen } from "@testing-library/react";
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
