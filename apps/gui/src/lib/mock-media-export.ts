// mock 媒体导出（仅 VITE_MOCK_IPC=1）：无真实对话框与文件系统，
// pickSavePath 模拟选到下载目录，exportMedia 三拍模拟进度，走通交互链路。
import type { MediaExportBackend } from "./ipc-types";

const MOCK_TOTAL_BYTES = 1024 * 1024;

export const mockMediaExportBackend: MediaExportBackend = {
  async pickSavePath(defaultFileName) {
    return `~/Downloads/${defaultFileName}`;
  },
  async exportMedia(sourceUrl, destPath, onProgress) {
    void sourceUrl; // mock 无真实来源
    for (const step of [1, 2, 3]) {
      onProgress({
        receivedBytes: (MOCK_TOTAL_BYTES / 3) * step,
        totalBytes: MOCK_TOTAL_BYTES,
      });
      await new Promise((resolve) => setTimeout(resolve, 120));
    }
    return { destPath, totalBytes: MOCK_TOTAL_BYTES };
  },
};
