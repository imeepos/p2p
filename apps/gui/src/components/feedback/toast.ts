import { toast } from "sonner";

const SUCCESS_DURATION_MS = 3000;
const ERROR_DURATION_MS = 6000;
const DEDUP_WINDOW_MS = 3000;
const DEDUP_MAP_PRUNE_SIZE = 100;

const recentKeys = new Map<string, number>();

function isDuplicate(key: string): boolean {
  const now = Date.now();
  if (recentKeys.size > DEDUP_MAP_PRUNE_SIZE) {
    for (const [k, at] of recentKeys) {
      if (now - at >= DEDUP_WINDOW_MS) recentKeys.delete(k);
    }
  }
  const last = recentKeys.get(key);
  if (last !== undefined && now - last < DEDUP_WINDOW_MS) return true;
  recentKeys.set(key, now);
  return false;
}

export function toastSuccess(message: string, description?: string) {
  if (isDuplicate("ok:" + message)) {
    return toast.dismiss();
  }
  return toast.success(message, {
    description,
    duration: SUCCESS_DURATION_MS,
  });
}

export function toastError(message: string, description?: string) {
  if (isDuplicate("err:" + message)) {
    return toast.dismiss();
  }
  return toast.error(message, { description, duration: ERROR_DURATION_MS });
}
