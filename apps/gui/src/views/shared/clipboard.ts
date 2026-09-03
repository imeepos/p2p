import { toastError, toastSuccess } from "@/components/feedback/toast";
import { errorText } from "@/views/shared/form-flow";

interface CopyMessages {
  done: string;
  failed: string;
}

// 剪贴板复制：成功/失败 toast + console 信号，返回是否成功供调用方分支。
export async function copyText(text: string, messages: CopyMessages): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    toastSuccess(messages.done);
    return true;
  } catch (error) {
    console.error("[views] 复制失败", error);
    toastError(messages.failed, { description: errorText(error) });
    return false;
  }
}
