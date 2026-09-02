import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

// 机械门禁：配置侧组件不得出现硬编码中文文案，必须经 i18n key 渲染。
// 范围：components/views/routes/hooks/config/theme（契约层 lib 与
// locale 资源本身除外）；console 日志是可观测信号而非 UI 文案，豁免。
const SRC_ROOT = join(process.cwd(), "src");
const SCOPE_DIRS = ["components", "views", "routes", "hooks", "config", "theme"];
const CJK = /[一-龥]/;

function walk(dir: string): string[] {
  if (!statSync(dir).isDirectory()) return [];
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? walk(path) : [path];
  });
}

function stripComments(source: string): string {
  return source
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/^\s*\/\/.*$/gm, "");
}

describe("i18n hardcoded copy scan", () => {
  it("views/shell carry no hardcoded CJK copy outside console logs", () => {
    const offenders: string[] = [];
    for (const dir of SCOPE_DIRS) {
      for (const file of walk(join(SRC_ROOT, dir))) {
        if (!file.endsWith(".tsx") && !file.endsWith(".ts")) continue;
        if (file.endsWith(".test.ts") || file.endsWith(".test.tsx")) continue;
        const lines = stripComments(readFileSync(file, "utf8")).split("\n");
        lines.forEach((line, index) => {
          // console/throw 是可观测信号与开发者误用守卫，不是界面文案。
          if (line.includes("console.") || line.includes("throw new Error(")) {
            return;
          }
          if (CJK.test(line)) offenders.push(`${file}:${index + 1}: ${line.trim()}`);
        });
      }
    }
    expect(offenders).toEqual([]);
  });
});
