// 中央登记守卫：menu.def 与 App.tsx 路由表一一对应，titleKey 必须在
// zh/en 两个 locale 资源中真实存在——登记悬空（菜单可见但路由/文案缺失）在此拦下。
import { readFileSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

import { MENU_ENTRIES } from "@/config/menu.def";
import { AcpPage } from "@/routes/acp-page";
import zhCN from "@/i18n/locales/zh-CN";
import enUS from "@/i18n/locales/en-US";

type Resource = Record<string, unknown>;

function hasPath(resource: Resource, key: string): boolean {
  let node: unknown = resource;
  for (const part of key.split(".")) {
    if (node === null || typeof node !== "object" || !(part in (node as Resource))) {
      return false;
    }
    node = (node as Resource)[part];
  }
  return typeof node !== "object" || node === null;
}

function appRoutePaths(): Set<string> {
  const source = readFileSync(join(process.cwd(), "src", "App.tsx"), "utf8");
  const paths = new Set<string>();
  if (/<Route index /.test(source)) paths.add("/");
  for (const match of source.matchAll(/path="([^"]+)"/g)) paths.add(match[1]);
  return paths;
}

describe("menu registration guard", () => {
  it("menu.def 每条路径都有对应 App 路由", () => {
    const routes = appRoutePaths();
    for (const entry of MENU_ENTRIES) {
      const routePath = entry.path === "/" ? "/" : entry.path.slice(1);
      expect(routes.has(routePath), entry.path + " 未在 App.tsx 登记").toBe(true);
    }
  });

  it("menu.def 每条 titleKey 在 zh/en 资源都存在", () => {
    for (const entry of MENU_ENTRIES) {
      expect(hasPath(zhCN, entry.titleKey), "zh 缺 " + entry.titleKey).toBe(true);
      expect(hasPath(enUS, entry.titleKey), "en 缺 " + entry.titleKey).toBe(true);
    }
  });

  it("ACP 视图经 /chat?kind=agent 保持可达（/acp 重定向保留）", () => {
    expect(routes().has("acp")).toBe(true);
    const chatRoute = readFileSync(join(process.cwd(), "src", "routes", "chat-route.tsx"), "utf8");
    expect(chatRoute).toContain("AcpPage");
    expect(typeof AcpPage).toBe("function");
  });
});

function routes(): Set<string> {
  return appRoutePaths();
}
