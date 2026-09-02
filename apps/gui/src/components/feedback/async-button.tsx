import { CheckIcon, LoaderCircleIcon, XIcon } from "lucide-react";
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";

import { Button, type ButtonProps } from "@/components/ui/button";

type AsyncStatus = "idle" | "loading" | "success" | "fail";

const MIN_LOADING_MS = 300;
const RESULT_HOLD_MS = 1200;

export interface AsyncButtonProps
  extends Omit<ButtonProps, "onClick" | "asChild" | "children"> {
  action: () => Promise<unknown>;
  onSuccess?: (result: unknown) => void;
  onError?: (error: unknown) => void;
  children?: ReactNode;
}

interface TimeoutBag {
  ids: number[];
}

function clearBag(bag: TimeoutBag) {
  bag.ids.forEach((id) => window.clearTimeout(id));
  bag.ids = [];
}

function defer(bag: TimeoutBag, fn: () => void, ms: number) {
  bag.ids.push(window.setTimeout(fn, ms));
}

function StatusIcon({ status }: { status: AsyncStatus }) {
  if (status === "loading") {
    return <LoaderCircleIcon className="animate-spin" aria-hidden />;
  }
  if (status === "success") {
    return <CheckIcon aria-hidden />;
  }
  if (status === "fail") {
    return <XIcon aria-hidden />;
  }
  return null;
}

export function AsyncButton({
  action,
  onSuccess,
  onError,
  children,
  disabled,
  ...props
}: AsyncButtonProps) {
  const [status, setStatus] = useState<AsyncStatus>("idle");
  const timers = useRef<TimeoutBag>({ ids: [] });

  useEffect(() => () => clearBag(timers.current), []);

  const handleClick = useCallback(async () => {
    if (status !== "idle") return;
    setStatus("loading");
    const startedAt = Date.now();
    const settle = (next: AsyncStatus, notify: () => void) => {
      const wait = Math.max(0, MIN_LOADING_MS - (Date.now() - startedAt));
      defer(timers.current, () => {
        setStatus(next);
        notify();
        defer(timers.current, () => setStatus("idle"), RESULT_HOLD_MS);
      }, wait);
    };
    try {
      const result = await action();
      settle("success", () => onSuccess?.(result));
    } catch (error) {
      settle("fail", () => onError?.(error));
    }
  }, [action, onError, onSuccess, status]);

  const busy = status !== "idle";

  return (
    <Button
      onClick={handleClick}
      disabled={disabled || busy}
      aria-busy={status === "loading"}
      {...props}
    >
      <StatusIcon status={status} />
      {children}
    </Button>
  );
}
