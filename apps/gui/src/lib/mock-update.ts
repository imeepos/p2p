import type {
  RemoteUpdate,
  UpdateCheckResult,
  UpdateDownloadBackend,
  UpdateDownloadProgress,
} from "./ipc-types";

// 契约 v4 §9 的 mock 侧三态：有更新（默认）/ 无更新 / 检查失败。
// VITE_MOCK_UPDATE=upToDate|failed 可在启动期切换；运行期直接改 mockUpdateFixture。
export type MockUpdateScenario = "available" | "upToDate" | "failed";

const MOCK_CURRENT_VERSION = "0.1.0";
const MOCK_LATEST_VERSION = "0.2.0";
const CHECK_DELAY_MS = 300;
const MOCK_PUBLISHED_AT_MS = Date.parse("2026-09-01T10:00:00Z");

const MOCK_RELEASE_URL =
  "https://github.com/imeepos/p2p/releases/tag/client-v0.2.0";
const MOCK_RELEASE_NOTES = [
  "## 亮点",
  "",
  "- 事件流支持按类型过滤与 JSON 导出",
  "- 拨号链新增逐跳耗时统计",
  "",
  "## 修复",
  "",
  "- 修复 mDNS 开关保存后未即时生效的问题",
  "- 修复深色主题下趋势图坐标轴颜色异常",
  "",
  "完整变更清单见仓库 CHANGELOG。",
].join("\n");

const envScenario = import.meta.env.VITE_MOCK_UPDATE;

export const mockUpdateFixture: {
  scenario: MockUpdateScenario;
  lastOpenedUrl: string | null;
  lastInstalledVersion: string | null;
  lastRelaunched: boolean;
} = {
  scenario:
    envScenario === "upToDate" || envScenario === "failed"
      ? envScenario
      : "available",
  lastOpenedUrl: null,
  lastInstalledVersion: null,
  lastRelaunched: false,
};

function upToDateResult(now: number): UpdateCheckResult {
  return {
    currentVersion: MOCK_CURRENT_VERSION,
    latestVersion: MOCK_CURRENT_VERSION,
    hasUpdate: false,
    releaseUrl: null,
    releaseName: null,
    releaseNotesMd: null,
    publishedAtMs: null,
    checkedAtMs: now,
  };
}

function availableResult(now: number): UpdateCheckResult {
  return {
    currentVersion: MOCK_CURRENT_VERSION,
    latestVersion: MOCK_LATEST_VERSION,
    hasUpdate: true,
    releaseUrl: MOCK_RELEASE_URL,
    releaseName: "p2p-console client-v0.2.0",
    releaseNotesMd: MOCK_RELEASE_NOTES,
    publishedAtMs: MOCK_PUBLISHED_AT_MS,
    checkedAtMs: now,
  };
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

export async function mockUpdateCheck(): Promise<UpdateCheckResult> {
  await delay(CHECK_DELAY_MS);
  const now = Date.now();
  if (mockUpdateFixture.scenario === "failed") {
    throw new Error("更新检查失败：无法访问 GitHub Releases（mock）");
  }
  return mockUpdateFixture.scenario === "upToDate"
    ? upToDateResult(now)
    : availableResult(now);
}

// 白名单校验对齐契约 §1：url 必须 https 且 host 为 github.com。
// mock 不真开浏览器，仅记录目标 url 供测试与演示断言。
export async function mockUpdateOpenReleasePage(url: string): Promise<void> {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    throw new Error("url 非法，无法打开发布页");
  }
  if (parsed.protocol !== "https:" || parsed.hostname !== "github.com") {
    throw new Error("url 必须 https 且 host 为 github.com");
  }
  mockUpdateFixture.lastOpenedUrl = url;
}

// 契约 v8 §13：下载安装 mock。48MB 假包按 24 块推进度，块间 50ms，
// 供浏览器 dev 演示进度条与测试断言字节推进。
const MOCK_DOWNLOAD_TOTAL_BYTES = 48 * 1024 * 1024;
const MOCK_DOWNLOAD_CHUNKS = 24;
const MOCK_DOWNLOAD_CHUNK_DELAY_MS = 50;

async function mockCheckRemoteUpdate(): Promise<RemoteUpdate | null> {
  await delay(CHECK_DELAY_MS);
  if (mockUpdateFixture.scenario === "failed") {
    throw new Error("更新检查失败：无法访问 GitHub Releases（mock）");
  }
  return mockUpdateFixture.scenario === "available"
    ? { version: MOCK_LATEST_VERSION, notes: null }
    : null;
}

async function mockDownloadAndInstallUpdate(
  onProgress: (p: UpdateDownloadProgress) => void,
): Promise<void> {
  await delay(CHECK_DELAY_MS);
  if (mockUpdateFixture.scenario !== "available") {
    throw new Error("无可用更新可下载安装（mock）");
  }
  const chunk = Math.ceil(MOCK_DOWNLOAD_TOTAL_BYTES / MOCK_DOWNLOAD_CHUNKS);
  let downloaded = 0;
  for (let i = 0; i < MOCK_DOWNLOAD_CHUNKS; i++) {
    await delay(MOCK_DOWNLOAD_CHUNK_DELAY_MS);
    downloaded = Math.min(downloaded + chunk, MOCK_DOWNLOAD_TOTAL_BYTES);
    onProgress({ downloadedBytes: downloaded, totalBytes: MOCK_DOWNLOAD_TOTAL_BYTES });
  }
  mockUpdateFixture.lastInstalledVersion = MOCK_LATEST_VERSION;
}

async function mockRelaunchApp(): Promise<void> {
  mockUpdateFixture.lastRelaunched = true;
}

export const mockUpdateDownloadBackend: UpdateDownloadBackend = {
  checkRemoteUpdate: mockCheckRemoteUpdate,
  downloadAndInstallUpdate: mockDownloadAndInstallUpdate,
  relaunchApp: mockRelaunchApp,
};
