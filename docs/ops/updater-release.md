# updater 发布与密钥运维（契约 v8 加法，G-U3）

应用内更新（下载+进度+自动安装）依赖签名增量包与 latest.json 清单，二者均由
gui-client.yml 流水线在 client-v* tag 时自动产出。本文只记人要做的事与不可逆风险。

## 密钥事实（2026-09-05 定因后固化，替代旧「无密码」说法）

- 本机私钥 `~/.tauri/p2p-console-updater.key` 为**空密码加密**的 rsign 私钥：
  文件本身是 348 字节、无换行的单行 base64，解码后首行为
  `untrusted comment: rsign encrypted secret key`。旧文档写「无密码」不准确——
  密钥是"已加密、口令为空"，与"未加密"在 tauri CLI 上行为不同（见对照表）。
- 因此本地任何签名动作必须**同时导出两个环境变量**（缺一即挂，见对照表）：

  ```bash
  set -a; source .env; set +a   # 提供 TAURI_SIGNING_PRIVATE_KEY_PATH（.env 只登记路径，不含密钥本体）
  export TAURI_SIGNING_PRIVATE_KEY="$(cat "$TAURI_SIGNING_PRIVATE_KEY_PATH")"
  export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""  # 空串也必须显式导出
  ```

## 常见报错对照表

| 报错（Tauri 打包 / tauri signer 步） | 原因 | 修法 |
|---|---|---|
| `failed to decode base64 secret key: Invalid symbol 37, offset 348` | 密钥内容不是合法 base64。2026-09-05 实证：GitHub secret 粘贴时带入尾随 `%`——密钥文件无尾换行，zsh 终端在输出后显示 EOL 标记 `%`，全选复制时一并带入；正确密钥恰 348 字符，故解码在 offset 348 撞上 ASCII 37(`%`) | 用 `cat` 文件原文重设 secret / 重导出变量，禁止从终端回显里复制 |
| `incorrect updater private key password: Device not configured (os error 6)` | `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 未导出。空密码密钥也要求显式导出空串，否则 tauri 走 TTY 口令提示，非交互 shell 直接失败 | `export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""` |
| `incorrect updater private key password: Wrong password for that key` | PASSWORD 导出了非空值/错值（incorrect password 类） | 改为空串导出 |
| `incorrect updater private key password: Missing comment in secret key` | KEY 输入为空串或不是密钥内容（如误传文件路径/截断） | 按 `$(cat "$TAURI_SIGNING_PRIVATE_KEY_PATH")` 重导出，跑预检核对逐字节一致 |

## 发布前预检（机械检查清单第 0 步，红=禁止继续）

```bash
set -a; source .env; set +a
export TAURI_SIGNING_PRIVATE_KEY="$(cat "$TAURI_SIGNING_PRIVATE_KEY_PATH")"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
bash scripts/ops/updater-signing-preflight.sh                # 期望：预检全绿 rc=0
bash scripts/ops/updater-signing-preflight.sh --self-test    # 改动脚本后必跑（9 场景红绿自检，防假绿）
```

预检覆盖：密钥文件可读、内容为严格 base64 且可解码、密钥头语义正确、
KEY/PASSWORD 双变量已导出、PASSWORD 为空串、KEY 与文件逐字节一致。
每个红项输出对应上表的真实报错，可直接对号。

## 一次性配置（已完成项打勾）

- [x] 本机生成 minisign 密钥对：`~/.tauri/p2p-console-updater.key(.pub)`（空密码加密）。
      公钥已入库：apps/gui/src-tauri/tauri.conf.json → plugins.updater.pubkey。
- [x] 本机 .env 登记 `TAURI_SIGNING_PRIVATE_KEY_PATH`（本地构建签名时 source .env 使用）。
- [x] **GitHub 仓库 secrets（需仓库管理员，一次即永久）**：
      - `TAURI_SIGNING_PRIVATE_KEY` = 私钥文件原文（`cat ~/.tauri/p2p-console-updater.key`，
        348 字节单行 base64）。2026-09-05 定因后已重写修复：旧值粘贴带入尾随 `%`，
        client-v0.1.4 / client-v0.1.5 两个 tag 四平台打包步同报
        `Invalid symbol 37, offset 348`，均无 release 出货。
      - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` = **不建**。GitHub secrets 存不了空串；
        secret 缺失时 workflow 的 env 表达式求值为空串导出，恰是空密码密钥所需。
      KEY 缺失时 build 矩阵在「Tauri 打包」步显式失败（createUpdaterArtifacts 机制兜底）。

## 发布流程（与既有 release.sh 流程叠加，不改变版本三处一致性门禁）

0. 签名环境预检（见上节），全绿才继续。
1. bump 版本：apps/gui/package.json / src-tauri/tauri.conf.json / src-tauri/Cargo.toml 三处同值
   （scripts/check/version.sh 机械拦截）。
2. tag：`client-vX.Y.Z`，只允许打在已合并 main 的提交（workflow 有 tag 祖先校验）。
3. CI 自动：构建四平台签名增量包 → release job 生成 latest.json（缺平台/缺签名直接 FAIL）
   → 附带全部产物发布 GitHub Release。
4. 端点 `releases/latest/download/latest.json` 恒指向最新 release 的清单，
   已装 0.1.4+ 客户端在启动/每 4h/手动检查时收到应用内更新提醒。

## 所有者侧 secret 核验与端到端验证

secret 值写后不可读回，用「形态核验 + 下个 tag 打包步变绿」闭环：

1. 核验项（repo Settings → Secrets and variables → Actions）：
   存在且仅存在 `TAURI_SIGNING_PRIVATE_KEY`；`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
   不应存在。KEY 的更新时间应晚于 2026-09-05T23:35Z（本次 API 重写）。
2. 期望值形态：`wc -c ~/.tauri/p2p-console-updater.key` = 348，单行无换行；
   重设时用 CLI（`gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/p2p-console-updater.key`）
   或 API（repo public key 做 sealed box 加密后 PUT），勿走终端复制粘贴。
3. 验证：bump 版本三处后推试验 tag `client-v0.1.6`（须打在 origin/main 祖先提交），
   在该 run 看：
   - 「诊断读取签名输入」步 success，条件步「①候选命中」「②候选命中」均 skipped；
   - 四平台「Tauri 打包」步 success（不再出现 `Invalid symbol`）；
   - release job success，GitHub Release 出现四平台 `*.tar.gz/.zip/.sig` 与 `latest.json`。
   任一步红则按报错对照表对号后重推 tag。

## 本地验证

- 签名构建：先过预检（见上），然后 `(cd apps/gui && pnpm exec tauri build)`，
  产物在 src-tauri/target/release/bundle/**（.app.tar.gz/.sig、*-setup.nsis.zip/.sig、
  *.AppImage.tar.gz/.sig）。
- 清单脚本干跑：构造假 artifacts 目录后
  `node apps/gui/scripts/release/make-latest-json.mjs --artifacts <dir> --tag client-vX.Y.Z --repo imeepos/p2p`。
- dev 模式（未打包二进制）不做真实安装；浏览器 dev 用 VITE_MOCK_IPC=1 走 mock 下载进度。

## 不可逆风险

私钥丢失 = 已装客户端的 updater 公钥作废，之后只能继续跳浏览器手动下载，
换新密钥必须随一次正常发版改 pubkey。请把私钥再备份一份到密码管理器/离线介质。

## 平台注意

- Windows：只有 NSIS（-setup.exe 对应 zip 增量包）走 updater；MSI 用户走浏览器。
- Linux：只有 AppImage 走 updater；deb 用户走浏览器。
- macOS：增量包为 .app.tar.gz，替换 bundle 后 relaunch；未签名（内部使用）不受影响。
