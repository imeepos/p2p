import { render, waitFor } from "@testing-library/react";
import { createRef } from "react";
import { describe, expect, it } from "vitest";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogOverlay,
  AlertDialogTitle,
} from "./alert-dialog";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogOverlay,
  DialogTitle,
  DialogTrigger,
} from "./dialog";

// Radix 会向 Overlay/Content 等透传 ref（焦点管理内部依赖），包装组件不转发
// 时 React 18 静默丢弃并告警（W6-S3 矩阵 F1）。断言 ref 落到对应 DOM 节点。
describe("dialog 家族 forwardRef", () => {
  it("AlertDialog Overlay/Content/Action/Cancel ref 落到 DOM", async () => {
    const overlayRef = createRef<HTMLDivElement>();
    const contentRef = createRef<HTMLDivElement>();
    const actionRef = createRef<HTMLButtonElement>();
    const cancelRef = createRef<HTMLButtonElement>();
    render(
      <AlertDialog open>
        <AlertDialogOverlay ref={overlayRef} />
        <AlertDialogContent ref={contentRef}>
          <AlertDialogTitle>标题</AlertDialogTitle>
          <AlertDialogDescription>描述</AlertDialogDescription>
          <AlertDialogCancel ref={cancelRef}>取消</AlertDialogCancel>
          <AlertDialogAction ref={actionRef}>确认</AlertDialogAction>
        </AlertDialogContent>
      </AlertDialog>,
    );
    await waitFor(() => expect(overlayRef.current).not.toBeNull());
    expect(overlayRef.current?.dataset.slot).toBe("alert-dialog-overlay");
    expect(contentRef.current?.dataset.slot).toBe("alert-dialog-content");
    expect(actionRef.current).toBeInstanceOf(HTMLButtonElement);
    expect(cancelRef.current).toBeInstanceOf(HTMLButtonElement);
  });

  it("Dialog Overlay/Content/Title/Trigger ref 落到 DOM", async () => {
    const overlayRef = createRef<HTMLDivElement>();
    const contentRef = createRef<HTMLDivElement>();
    const titleRef = createRef<HTMLHeadingElement>();
    const triggerRef = createRef<HTMLButtonElement>();
    render(
      <Dialog open>
        <DialogOverlay ref={overlayRef} />
        <DialogTrigger ref={triggerRef}>打开</DialogTrigger>
        <DialogContent ref={contentRef}>
          <DialogTitle ref={titleRef}>标题</DialogTitle>
          <DialogDescription>描述</DialogDescription>
        </DialogContent>
      </Dialog>,
    );
    await waitFor(() => expect(overlayRef.current).not.toBeNull());
    expect(overlayRef.current?.dataset.slot).toBe("dialog-overlay");
    expect(contentRef.current?.dataset.slot).toBe("dialog-content");
    expect(titleRef.current?.dataset.slot).toBe("dialog-title");
    expect(triggerRef.current?.dataset.slot).toBe("dialog-trigger");
  });
});

