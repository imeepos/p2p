// 关联选择器的统一选项模型：value = 稳定标识（PeerId），label = 人可读名，
// hint = 缩略标识副行。视图层负责从各自 store 组装，选择器只认这一个形状。
export interface PickerOption {
  value: string;
  label: string;
  hint?: string;
}

// 缩略统一走全站 6+4 口径（R2-17），不再保留第二套头12尾8实现。
export { shortPeerId } from "@/lib/peer-name";

// 即时搜索：名称/标识不区分大小写的包含匹配，空查询返回全量。
export function filterOptions(options: PickerOption[], query: string): PickerOption[] {
  const q = query.trim().toLowerCase();
  if (q.length === 0) return options;
  return options.filter(
    (option) =>
      option.label.toLowerCase().includes(q) || option.value.toLowerCase().includes(q),
  );
}
