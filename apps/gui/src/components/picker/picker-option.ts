// 关联选择器的统一选项模型：value = 稳定标识（PeerId），label = 人可读名，
// hint = 缩略标识副行。视图层负责从各自 store 组装，选择器只认这一个形状。
export interface PickerOption {
  value: string;
  label: string;
  hint?: string;
}

// PeerId 缩略：头 12 + 尾 8（头长对齐 PEER_ID_PREFIX_LEN 口径），中段省略。
export function shortPeerId(peerId: string, head = 12, tail = 8): string {
  if (peerId.length <= head + tail + 1) return peerId;
  return peerId.slice(0, head) + "\u2026" + peerId.slice(-tail);
}

// 即时搜索：名称/标识不区分大小写的包含匹配，空查询返回全量。
export function filterOptions(options: PickerOption[], query: string): PickerOption[] {
  const q = query.trim().toLowerCase();
  if (q.length === 0) return options;
  return options.filter(
    (option) =>
      option.label.toLowerCase().includes(q) || option.value.toLowerCase().includes(q),
  );
}
