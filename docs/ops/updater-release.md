# updater 发布与密钥运维（契约 v8 加法，G-U3）

应用内更新（下载+进度+自动安装）依赖签名增量包与 latest.json 清单，二者均由
gui-client.yml 流水线在 client-v* tag 时自动产出。本文只记人要做的事与不可逆风险。

## 一次性配置（已完成项打勾）

- [x] 本机生成 minisign 密钥对：`~/.tauri/p2p-console-updater.key(.pub)`（无密码）。
      公钥已入库：apps/gui/src-tauri/tauri.conf.json → plugins.updater.pubkey。
- [x] 本机 .env 登记 `TAURI_SIGNING_PRIVATE_KEY_PATH`（本地构建签名时 source .env 使用）。
- [ ] **GitHub 仓库 secrets（需仓库管理员，一次即永久）**：
      - `TAURI_SIGNING_PRIVATE_KEY` = 私钥文件全文（`cat ~/.tauri/p2p-console-updater.key`）
      - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` = 留空不建（当前密钥无密码）
      缺失时 build 矩阵在「Tauri 打包」步显式失败（createUpdaterArtifacts 机制兜底）。

## 不可逆风险

私钥丢失 = 已装客户端的 updater 公钥作废，之后只能继续跳浏览器手动下载，
换新密钥必须随一次正常发版改 pubkey。请把私钥再备份一份到密码管理器/离线介质。

## 发布流程（与既有 release.sh 流程叠加，不改变版本三处一致性门禁）

1. bump 版本：apps/gui/package.json / src-tauri/tauri.conf.json / src-tauri/Cargo.toml 三处同值
   （scripts/check/version.sh 机械拦截）。
2. tag：`client-vX.Y.Z`，只允许打在已合并 main 的提交（workflow 有 tag 祖先校验）。
3. CI 自动：构建四平台签名增量包 → release job 生成 latest.json（缺平台/缺签名直接 FAIL）
   → 附带全部产物发布 GitHub Release。
4. 端点 `releases/latest/download/latest.json` 恒指向最新 release 的清单，
   已装 0.1.4+ 客户端在启动/每 4h/手动检查时收到应用内更新提醒。

## 本地验证

- 签名构建：`source .env && (cd apps/gui && pnpm exec tauri build)`，
  产物在 src-tauri/target/release/bundle/**（.app.tar.gz/.sig、*-setup.nsis.zip/.sig、
  *.AppImage.tar.gz/.sig）。
- 清单脚本干跑：构造假 artifacts 目录后
  `node apps/gui/scripts/release/make-latest-json.mjs --artifacts <dir> --tag client-vX.Y.Z --repo imeepos/p2p`。
- dev 模式（未打包二进制）不做真实安装；浏览器 dev 用 VITE_MOCK_IPC=1 走 mock 下载进度。

## 平台注意

- Windows：只有 NSIS（-setup.exe 对应 zip 增量包）走 updater；MSI 用户走浏览器。
- Linux：只有 AppImage 走 updater；deb 用户走浏览器。
- macOS：增量包为 .app.tar.gz，替换 bundle 后 relaunch；未签名（内部使用）不受影响。
