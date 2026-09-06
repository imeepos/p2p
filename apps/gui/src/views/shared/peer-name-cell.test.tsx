import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import "@/i18n";
import type { ChatFriendJson } from "@/lib/ipc-types";
import { useChatStore } from "@/stores/chat-store";
import { PeerNameCell } from "./peer-name-cell";

const PEER = "a".repeat(40) + "Zzzz";

const friend: ChatFriendJson = {
  peerId: PEER,
  nickname: "阿北",
  addrs: [],
  note: "老朋友",
};

describe("PeerNameCell 首列人话化（F02）", () => {
  beforeEach(() => {
    useChatStore.setState({ friends: [], friendsLoaded: true });
  });

  it("已知好友：昵称为主行，缩略 ID 为副行，title 挂完整 ID", () => {
    useChatStore.setState({ friends: [friend], friendsLoaded: true });
    render(<PeerNameCell peerId={PEER} />);
    expect(screen.getByText("阿北")).toBeInTheDocument();
    expect(screen.getByText("aaaaaa…Zzzz")).toBeInTheDocument();
    expect(screen.getByTitle(PEER)).toBeInTheDocument();
  });

  it("昵称为空的好友回退备注作主行", () => {
    useChatStore.setState({
      friends: [{ ...friend, nickname: "" }],
      friendsLoaded: true,
    });
    render(<PeerNameCell peerId={PEER} />);
    expect(screen.getByText("老朋友")).toBeInTheDocument();
  });

  it("非好友仅缩略 ID 单行", () => {
    render(<PeerNameCell peerId={PEER} />);
    expect(screen.getByText("aaaaaa…Zzzz")).toBeInTheDocument();
    expect(screen.queryByText("阿北")).toBeNull();
  });
});
