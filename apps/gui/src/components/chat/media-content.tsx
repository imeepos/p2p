import { FileText } from "lucide-react";
import { useTranslation } from "react-i18next";

import type { Locale } from "@/i18n";
import { formatBytes } from "@/lib/format";
import type { ChatMediaJson } from "@/lib/ipc-types";

import { MediaDownload } from "./media-download";

// 可内联资源：真实后端经 asset protocol 返回 http(s) 地址；本地 blob/data 兜底。
function inlineSrc(path?: string | null): string | null {
  if (!path) return null;
  return /^(https?:|blob:|data:|asset:)/.test(path) ? path : null;
}

interface MediaContentProps {
  media: ChatMediaJson;
}

// 图片/音频/视频可内联时直接渲染；占位路径（mock）退化为信息卡 + 导出入口（§12.5）。
export function MediaContent({ media }: MediaContentProps) {
  const { i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const src = inlineSrc(media.path);

  if (src && media.mime.startsWith("image/")) {
    return <img src={src} alt={media.name} className="max-h-64 max-w-full rounded-md" />;
  }
  if (src && media.mime.startsWith("audio/")) {
    return <audio controls src={src} className="max-w-full" />;
  }
  if (src && media.mime.startsWith("video/")) {
    return <video controls src={src} className="max-h-64 max-w-full rounded-md" />;
  }

  return (
    <div className="flex items-center gap-2 text-sm">
      <FileText className="size-4 shrink-0" aria-hidden />
      <div className="min-w-0">
        <div className="truncate">{media.name}</div>
        <div className="text-xs opacity-70">
          {formatBytes(media.size, locale)}
          {media.path ? <MediaDownload media={media} /> : null}
        </div>
      </div>
    </div>
  );
}
