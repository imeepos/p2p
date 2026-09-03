import { fileURLToPath, URL } from "node:url";

import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

import { version } from "./package.json";

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  // 与 vite.config.ts 的 define 保持一致：build-time 注入的全局量
  // 在测试环境缺失时是渲染期 ReferenceError（整树崩溃、白屏级）。
  define: { __APP_VERSION__: JSON.stringify(version) },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    include: ["src/**/*.test.{ts,tsx}"],
  },
});
