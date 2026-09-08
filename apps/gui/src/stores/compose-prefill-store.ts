import { create } from "zustand";

// composer 预填交接（分享链接「发送到聊天」）：?compose= 深链把链接文本暂存于此，
// Composer 挂载时一次性消费填入输入框；用户在会话列表中自行选择接收方。
interface ComposePrefillState {
  text: string | null;
  setText: (text: string) => void;
  /** 一次性消费：取走并清空（防重复预填到多个会话） */
  consume: () => string | null;
}

export const useComposePrefillStore = create<ComposePrefillState>((set, get) => ({
  text: null,
  setText: (text) => set({ text }),
  consume: () => {
    const text = get().text;
    set({ text: null });
    return text;
  },
}));
