import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import { FACTORY_LIST_DEFAULTS } from "./factory-defaults";

// 出厂默认端点防漂移：views/shared/factory-defaults.ts 是后端
// apps/gui/src-tauri/src/config.rs default_* 的展示层镜像。本测试直接解析
// Rust 权威源里的 vec! 字面量对表，任一侧漂移立即红。
// 互指注释：factory-defaults.ts 头部注明权威源为 config.rs。
const RUST_SOURCE_PATH = join(process.cwd(), "src-tauri/src/config.rs");

interface RustDefaults {
  bootstrap: string[];
  relayAddrs: string[];
  observationAddrs: string[];
}

function extractVec(source: string, fnName: string): string[] {
  const fnIndex = source.indexOf("fn " + fnName + "()");
  if (fnIndex < 0) {
    throw new Error(
      "config.rs 中未定位到 fn " + fnName + "，解析器需随源码结构调整同步更新",
    );
  }
  const vecStart = source.indexOf("vec![", fnIndex);
  const bodyEnd = source.indexOf("}", fnIndex);
  if (vecStart < 0 || vecStart > bodyEnd) {
    throw new Error("fn " + fnName + " 体内未找到 vec![ 字面量");
  }
  const vecEnd = source.indexOf("]", vecStart);
  if (vecEnd < 0) {
    throw new Error("fn " + fnName + " 的 vec![ 缺少闭合 ]");
  }
  const literals = source.slice(vecStart + 5, vecEnd).match(/"([^"]*)"/g);
  if (!literals) {
    throw new Error("fn " + fnName + " 的 vec! 内未解析到字符串字面量");
  }
  return literals.map((raw) => raw.slice(1, -1));
}

function readRustDefaults(): RustDefaults {
  const source = readFileSync(RUST_SOURCE_PATH, "utf8");
  return {
    bootstrap: extractVec(source, "default_bootstrap"),
    relayAddrs: extractVec(source, "default_relay_addrs"),
    observationAddrs: extractVec(source, "default_observation_addrs"),
  };
}

describe("factory defaults mirror config.rs", () => {
  it("解析器能从权威源提取三组默认端点（防解析器静默失配）", () => {
    const rust = readRustDefaults();
    expect(rust.bootstrap.length).toBeGreaterThan(0);
    expect(rust.relayAddrs.length).toBeGreaterThan(0);
    expect(rust.observationAddrs.length).toBeGreaterThan(0);
  });

  it("bootstrap 镜像与 config.rs default_bootstrap 一致", () => {
    expect(FACTORY_LIST_DEFAULTS.bootstrap).toEqual(readRustDefaults().bootstrap);
  });

  it("relayAddrs 镜像与 config.rs default_relay_addrs 一致", () => {
    expect(FACTORY_LIST_DEFAULTS.relayAddrs).toEqual(
      readRustDefaults().relayAddrs,
    );
  });

  it("observationAddrs 镜像与 config.rs default_observation_addrs 一致", () => {
    expect(FACTORY_LIST_DEFAULTS.observationAddrs).toEqual(
      readRustDefaults().observationAddrs,
    );
  });
});
