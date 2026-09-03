import { useTranslation } from "react-i18next";

// 常用 unicode emoji 面板（需求：约 20-40 个，不做分类页）。
const EMOJIS = [
  "😀", "😄", "😂", "🥲", "😊", "😍", "🤔", "😎",
  "😭", "😅", "😴", "🥳", "😡", "👍", "👎", "🙏",
  "💪", "👏", "🎉", "❤️", "🔥", "✨", "🌟", "☀️",
  "🌙", "⭐", "🍀", "🎂", "🚀", "🎵", "⚡", "✅",
] as const;

interface EmojiPickerProps {
  onPick: (emoji: string) => void;
}

export function EmojiPicker({ onPick }: EmojiPickerProps) {
  const { t } = useTranslation();
  return (
    <div
      role="menu"
      aria-label={t("chat.emoji")}
      className="grid max-w-56 grid-cols-8 gap-1 rounded-md border bg-background p-2"
    >
      {EMOJIS.map((emoji) => (
        <button
          key={emoji}
          type="button"
          role="menuitem"
          onClick={() => onPick(emoji)}
          className="rounded p-1 text-lg hover:bg-accent"
        >
          {emoji}
        </button>
      ))}
    </div>
  );
}
