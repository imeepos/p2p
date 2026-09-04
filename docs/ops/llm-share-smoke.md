# llm-share 真两机联调冒烟（T23）

> 对应脚本：\u0060scripts/ops/llm-share-smoke.sh\u0060（单脚本编排，末行全绿输出 \u0060SMOKE-OK\u0060）。
> 设计依据：[idle-token-sharing-plan](../design/idle-token-sharing-plan.md) §10 验收口径
> A1-A6 的真两机子集；Mina rust 节点测试分类（rust_to_rust / discovery_seed /
> bad_node）+ libp2p interop 测试纪律（场景矩阵、幂等重跑、产物清理）。

## 1. 前置条件

| 项 | 要求 |
| --- | --- |
| 本机 | macOS，\u0060gtimeout\u0060（brew coreutils）、\u0060cargo\u0060、\u0060git\u0060；LAN IP 可路由（默认取 en0/en1，兜底 192.168.0.15） |
| 远端 | \u0060imeepos@192.168.0.102\u0060（AGENTS.md 授权；可用 \u0060LLM_SMOKE_REMOTE\u0060/\u0060LLM_SMOKE_REMOTE_IP\u0060 覆盖） |
| SSH | 免密（BatchMode）可达；远端可出网访问 crates.io（rustup/依赖下载；github 不可达也没关系，代码经 git bundle 供给） |
| 端口 | 本机 UDP 35410（rendezvous bootstrap）+ 35412（观测反射口）；102 UDP 35420（出借方 QUIC）。被占即失败，须人工释放（按端口查进程，禁 pkill 模式匹配） |
| 密钥 | 不需要——上游是进程内 mock，无真实 API key；节点身份种子由脚本生成的 harness 落盘（隔离目录内） |

## 2. 用法

\u0060```bash\u0060
bash scripts/ops/llm-share-smoke.sh
\u0060```\u0060

- 全程约 10 分钟（首次，含两端 cargo 构建）；二连跑第二次全部命中缓存，约 1-2 分钟。
- 逐场景输出 \u0060Sx PASS ...\u0060；任一 FAIL 立即失败退出；末行全绿输出 \u0060SMOKE-OK\u0060。
- 幂等：连跑两次全绿（第二次不得因残留数据失败）。脚本总超时 1740s 自毁（低于建议上限 1800s）。

## 3. 场景清单

| 场景 | 断言 | 机制 |
| --- | --- | --- |
| S1 远端供给 | 两端工具链/代码/harness/p2pctl 就绪 | 探测 102 cargo（缺失则 rustup 装至其 $HOME，禁 sudo）；本仓库 HEAD 打 git bundle → scp → 102 端 clone/增量 fetch+reset 到 \u0060~/llm-smoke-work/src\u0060；两端 cargo 构建 harness（脚本生成的独立夹具，路径依赖产品 crate）与 p2pctl |
| S2 互联 | rendezvous 查号命中 + 跨机连接建立 | 本机拉起 bootstrap（facade 节点内置 rendezvous 服务端 + 观测反射口），102 出借方注册（观测学到的 LAN 地址 + 监听端口），借方 \u0060query_peer\u0060 发现后 connect |
| S3 出借方=102 | 声明验签 + TTL + 选路 | 102 侧以节点身份跑产品命令 \u0060p2pctl llm-share offer publish\u0060 签名发布；本机借方经 \u0060/llm-share/offer/1\u0060 取回信封，OfferBook::insert（Ed25519 签名 + issued_at/TTL 时间窗）+ select_offers 命中 102 |
| S4 跨机调用 | 流式回包 + 双边各记一笔 + 收据验签 PASS | 借方发 OpenAI 格式流式请求（req_id/max_tokens），102 侧三闸准入后经进程内 mock 上游 SSE 逐帧转发；收据 Ed25519 验签：harness（llm-share-ledger）与产品 CLI \u0060p2pctl llm-share receipt verify\u0060 双验；借方 \u0060ledger.json\u0060（产品格式）+ 出借方进程账本各记一笔且净差一致；上游调用计数=1 |
| S5 负向 | 结构化拒绝 + 上游零调用 | 第二借方身份（不在 allowlist）同链路请求 → \u0060REJECT-OK code=NotAllowlisted\u0060；上游调用计数增量 0、出借方账本不新增 |
| S6 可观察 | 坏节点/断流不挂死 + 显式错误 | S6a 对未注册 PeerId 查号：显式 \u0060DISCOVER-FAIL\u0060 非零退出（有界等待）；S6b 断流剧本（第 2 次上游调用在 usage 帧前切断）：显式 \u0060STREAM-BROKEN\u0060 + \u0060estimated: true\u0060 收据且验签 PASS |

## 4. harness（脚本生成夹具）与产品边界

\u0060/llm-share/proxy\u0060 与 \u0060/llm-share/offer\u0060 的进程级装配（底座 handler 注册、allowlist/模型路由配置）属产品待接线面（Phase 1），当前仅存在于 itest 进程内夹具。本冒烟不越界改产品代码，由脚本在运行目录生成独立 harness cargo 工程（不写进仓库）：

- 路径依赖 \u0060llm-share-proxy\u0060/\u0060llm-share-offer\u0060/\u0060llm-share-ledger\u0060/\u0060p2p\u0060 等产品 crate；
- 出借方 serve：底座 Node + offer/proxy 协议 handler + 进程内 mock 上游（调用计数落元数据日志，逐调用一行，仅含序号/模型名）；
- 借方 call：rendezvous 发现 → offer 验签选路 → 代理流式调用 → 收据验签入账 → 收据/账本落盘（产品 wire 格式，供产品 CLI 直接读取）；
- 签名/验签/账本/收据/选路均为产品 crate 真实逻辑；S3 发布与 S4/S6 收据验签分别使用产品命令 offer publish / receipt verify。

## 5. 清理说明

- 本地：\u0060$TMPDIR/llm-smoke-run-<pid>\u0060（每轮唯一：日志/借方身份/收据）与 \u0060$TMPDIR/llm-smoke-harness\u0060（夹具构建缓存，llm-smoke- 前缀隔离）。
- 远端：\u0060~/llm-smoke-work/\u0060（src/harness 构建缓存保留以加速重跑；lender/、logs/、run/ 每轮重建，退出时清理）。
- 退出路径（含失败/超时 TERM）均经 trap 清理：本地 kill 精确记录的 PID + 删目录；远端按 \u0060run/*.pid\u0060 精确终结（红线：绝不按进程名/模式 pkill）后删目录。
- 元数据日志（上游调用计数、SERVE-LEDGER 行）随目录清理；留存在的亦不影响幂等。prompt/回答内容不落盘：mock 回答为固定帧且不打印载荷，日志仅元数据。
- 人工全清：\u0060rm -rf /tmp/llm-smoke-*\u0060；102 侧 \u0060rm -rf ~/llm-smoke-work\u0060。

## 6. 已知边界与排查

- S2 发现失败优先查两端防火墙（UDP 35410/35412/35420）与 RUST_LOG=warn 下 "no routable addr" 类 WARN（观测反射未命中时注册地址不可路由，跨机查号即空）。
- 首次供给远端构建失败多为 crates.io 不可达（依赖下载）——102 需能出网到 crates.io（github 不通不影响，代码走 bundle）。
- 断流收据 \u0060estimated: true\u0060 的前提是切断发生在 usage 帧之前（A6 语义：上游已给 usage 则按实际计费），mock 剧本已按此编排。
- 坏状态快速自愈：任何一轮失败后直接重跑即可（每轮身份/账本/声明全部新建；远端代码经 bundle fetch+reset 强制对齐本仓库 HEAD）。
