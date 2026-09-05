import { fileURLToPath, URL } from "node:url";

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig, type Plugin } from "vite";
import { version } from "./package.json";

// 硬约束（2026-09-05 审计）：生产产物禁止 mock。Vite 中 shell 环境变量优先于
// .env 文件：发布时 shell 带 VITE_MOCK_IPC=1 或误用 --mode development 构建，
// mock 后端会全量打进 bundle 且现有门禁均不拦截。此断言在 configResolved 阶段
// 即抛错（bundle 尚未开始，泄漏构建秒级红）；产物侧第二道网见
// scripts/check/gui-dist-scan.sh，护栏自测见 scripts/check/tests/mock-ipc-guards.sh。
function noMockInBuild(): Plugin {
  return {
    name: "no-mock-in-build",
    configResolved(config) {
      if (config.command === "build" && config.env.VITE_MOCK_IPC === "1") {
        throw new Error(
          "no-mock-in-build: VITE_MOCK_IPC=1 禁止进入 build 产物（生产不许出现假数据）；" +
            "请清空 shell 的 VITE_MOCK_IPC，开发态用 vite dev。",
        );
      }
    },
  };
}

export default defineConfig({
  plugins: [react(), tailwindcss(), noMockInBuild()],
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  define: { __APP_VERSION__: JSON.stringify(version) },
  build: { target: "es2022" },
  clearScreen: false,
  server: { port: 5173, strictPort: true },
});
