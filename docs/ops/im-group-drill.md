# IM 群聊人工演练清单（G5 配套）

> 对应自动化：`cargo test -p p2p-itest --test chat_group_e2e`（同序列机械断言）。
> 本清单供跨机真环境演练用：三个数据目录（A=owner、B、C），同一版本 p2pctl。
> 约定：`$D<x>` 为第 x 方数据目录；所有命令加 `--json` 可得单行紧凑 JSON 便于留证。
> 通用退出准则：每步命令退出码与回显字段满足"期望"，任何 FAIL/异常 stderr 均记入演练纪要。

## 0. 前置

- [ ] 选定拓扑并全程保持：三方 serve 常驻 / 混合 / 纯一次性。拓扑与 D6 身份互斥的
      相容性先读 §0.5 矩阵；重邀回归对 owner 拓扑有硬要求，见 §5.1
- [ ] 互加好友：A↔B、A↔C（`p2pctl chat friends add --peer-id <PEER> --addr <ip/u端口>`）
- [ ] `p2pctl chat friends list --data-dir $D<a>` 可见 B/C 与地址

## 0.5 D6 身份互斥操作矩阵（2026-09-05 三节点实跑验证）

同 data-dir 的 chat 域进程受 `identity.lock` 互斥（机理见 p2pctl-ai-guide §1.3）。
本方 `chat serve` 常驻持锁时，另开进程执行本方命令的相容性：

| 类别 | 命令 | 与本方 serve 并存 | 依据 |
|---|---|---|---|
| 只读本地 | `group list` / `group history` / `group media file` / `chat history` / `chat friends list` | 可 | 不取身份锁、不起投递节点，只读本地库 |
| 网络型（持锁） | `group send` / `chat send` | 不可：立即退出 1「身份被占用」 | 取 identity.lock，持锁即拒（2026-09-05 演练实录） |
| 网络型（其余写操作） | `group create` / `invite` / `kick` / `leave` / `rename` / `disband` | 不可：按「停 serve 后执行」处理 | 不持锁，但与 serve 双进程共写同一聊天库与 goutbox，并存行为未验证，按 D6 一律互斥 |

序列约定：本方网络型命令一律「停 serve → 执行 → 需要收信再重启 serve」；只读命令
随时可跑。三方纯一次性（命令两两错峰、执行时对端在线）也成立，但 owner 纯一次性
会封锁成员→owner 通知通路（§5.1），重邀回归不适用该拓扑。

## 1. 建群与 roster 下发

- [ ] A：`p2pctl group create --name 演练群 --member <B> --member <C>`
      期望：退出 0；JSON groupId/rev=0/state=active/members 含 owner+2 成员
- [ ] B/C：`p2pctl group list` 期望出现该群（owner=A、state=active）

## 2. 文本 fan-out 与送达明细

- [ ] A：`p2pctl group send --group <GID> --text "第一条"`
      期望：`acked 2/2 delivered=true`；退出 0
- [ ] B/C：`p2pctl group history --group <GID>` 期望含"第一条"，sender=A

## 3. 附件 roundtrip

- [ ] A：`p2pctl group send --group <GID> --file ./shot.png --kind image`
      期望：delivered=true；media.name 为裸文件名 shot.png（basename，非全路径）
- [ ] B/C：`p2pctl group media file --group <GID> --message <MSG_ID>`
      期望：路径在 `media/<GID>/` 下，文件字节与原件一致（`shasum` 比对）

## 4. 离线成员上线 flush

- [ ] 停 C 的常驻进程；A：`p2pctl group send --group <GID> --text "离线补投"`
      期望：`acked 1/2 delivered=false`、退出 1（R4：未全员送达不假成功）
- [ ] 重启 C（同 data-dir）；C 重连 A 后等待 flush
      实测口径（2026-09-05）：补投可能滞后——紧邻的后续发送不触发补投，隔数次
      命令后才送达（ISSUE 2026-09-05 补投竞态，修复前勿记假失败）；判据=在后续
      若干次命令窗口内最终出现
- [ ] C：`p2pctl group history --group <GID>` 期望含"离线补投"
- [ ] A：`p2pctl group history --group <GID>` 期望该消息 acks 含 B 与 C
      实测口径：sender 侧 acks 在补投成功后不回填（ISSUE 2026-09-05 记账脱节），
      修复前以收端 history 为送达判据，A 侧 acks 缺员不记失败

## 5. kick / leave / 重邀回归

- [ ] A：`p2pctl group kick --group <GID> --member <B>` 期望 rev+1、名单收缩
- [ ] B：`p2pctl group list` 期望该群 state=kicked；`p2pctl group send` 退出 1（禁止发送）
- [ ] C：`p2pctl group leave --group <GID>` 期望 state=left；A 名单收缩（rev+1）。
      注意：A 名单收缩依赖 C 的 G_LEAVE 到达 owner，owner 纯一次性拓扑下不可达，
      先按 §5.1 选定通路再执行本步
- [ ] C：`p2pctl group history --group <GID>` 期望历史保留
- [ ] A：`p2pctl group invite --group <GID> --member <C>` 期望 C 端 state 回 active
      （重邀回归）。撞「已在群中」= G_LEAVE 未达 owner 的信号，按 §5.1 兜底处理

### 5.1 重邀回归通路（2026-09-05 修订，替代原无条件重邀）

成因：G_LEAVE 只发给 owner。owner 纯一次性拓扑下成员簿没有可拨的 owner 常驻地址，
G_LEAVE 滞留成员 goutbox——一次性进程退出即停止重试，owner 事后拨入也不触发
flush（flush 只挂在出站 connect）——owner roster 视图里 C 仍在群，此时 invite
必撞「已在群中」假错误。按拓扑三选一：

- [ ] 首选：owner `chat serve` 常驻，成员簿登记 owner 常驻地址；成员一次性命令可
      拨达 owner，G_LEAVE 即时补投。owner 自身执行持锁命令（send）时按 §0.5 矩阵
      停 serve → 执行 → 重启 serve
- [ ] 次选：成员簿预置 owner 地址（owner 曾以固定端口 serve 且地址仍有效期间），
      效果同上
- [ ] 兜底：不重试 invite，改「解散重建」：A `group disband` → `group create` 重建 →
      重新邀请；旧群历史保留（解散不删数据），后续断言以新 GID 为准

若坚持 owner 纯一次性拓扑：跳过「C leave → A invite」回归对，仅保留 kick 回归，
并在纪要标注 G_LEAVE 不可达为本拓扑已知边界（非缺陷）。

## 6. 解散

- [ ] A：`p2pctl group disband --group <GID>` 期望本端 state=disbanded
- [ ] C：`p2pctl group list` 期望 state=disbanded；`p2pctl group send` 退出 1
- [ ] A/C：`p2pctl group history --group <GID>` 期望历史保留（解散不删数据）

## 7. 收尾

- [ ] 演练纪要归档（各步退出码与关键回显）
- [ ] 演练数据目录按需清理（`rm -rf $D<x>`，含身份与聊天库）

## 已知边界（演练中预期行为，勿记缺陷）

- 成员离线期间 create/invite/kick/leave 的 roster/通知经 goutbox 补投，命令退出 0。
  已知缺陷（ISSUE 2026-09-05）：对某成员 connection lost 后该条目滞留
  status=failed 不自动重试，该成员停留旧 rev 直至下一次 roster bump（高 rev 胜
  最终一致收敛）；演练中见成员视图过期先查发端 goutbox。
- unknown_group：成员未建群前收到群消息 → 对端回 ACK(reason=unknown_group)，
  发端条目保持 pending 等 roster 补投；若随后 roster 仍未达，条目持续 pending 并留告警。
- 身份互斥：同 data-dir 同时仅一个进程持有身份，命令级相容性按 §0.5 矩阵执行。
