# IM 群聊人工演练清单（G5 配套）

> 对应自动化：`cargo test -p p2p-itest --test chat_group_e2e`（同序列机械断言）。
> 本清单供跨机真环境演练用：三个数据目录（A=owner、B、C），同一版本 p2pctl。
> 约定：`$D<x>` 为第 x 方数据目录；所有命令加 `--json` 可得单行紧凑 JSON 便于留证。
> 通用退出准则：每步命令退出码与回显字段满足"期望"，任何 FAIL/异常 stderr 均记入演练纪要。

## 0. 前置

- [ ] 三方各自 `p2pctl chat serve --data-dir $D<x>` 常驻（或以一次性命令+对方在线拓扑执行）
- [ ] 互加好友：A↔B、A↔C（`p2pctl chat friends add --peer-id <PEER> --addr <ip/u端口>`）
- [ ] `p2pctl chat friends list --data-dir $D<a>` 可见 B/C 与地址

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
      期望：delivered=true
- [ ] B/C：`p2pctl group media file --group <GID> --message <MSG_ID>`
      期望：路径在 `media/<GID>/` 下，文件字节与原件一致（`shasum` 比对）

## 4. 离线成员上线 flush

- [ ] 停 C 的常驻进程；A：`p2pctl group send --group <GID> --text "离线补投"`
      期望：`acked 1/2 delivered=false`、退出 1（R4：未全员送达不假成功）
- [ ] 重启 C（同 data-dir）；C 重连 A 后等待 flush
- [ ] C：`p2pctl group history --group <GID>` 期望含"离线补投"
- [ ] A：`p2pctl group history --group <GID>` 期望该消息 acks 含 B 与 C

## 5. kick / leave / 重邀回归

- [ ] A：`p2pctl group kick --group <GID> --member <B>` 期望 rev+1、名单收缩
- [ ] B：`p2pctl group list` 期望该群 state=kicked；`p2pctl group send` 退出 1（禁止发送）
- [ ] C：`p2pctl group leave --group <GID>` 期望 state=left；A 名单收缩（rev+1）
- [ ] C：`p2pctl group history --group <GID>` 期望历史保留
- [ ] A：`p2pctl group invite --group <GID> --member <C>` 期望 C 端 state 回 active（重邀回归）

## 6. 解散

- [ ] A：`p2pctl group disband --group <GID>` 期望本端 state=disbanded
- [ ] C：`p2pctl group list` 期望 state=disbanded；`p2pctl group send` 退出 1
- [ ] A/C：`p2pctl group history --group <GID>` 期望历史保留（解散不删数据）

## 7. 收尾

- [ ] 演练纪要归档（各步退出码与关键回显）
- [ ] 演练数据目录按需清理（`rm -rf $D<x>`，含身份与聊天库）

## 已知边界（演练中预期行为，勿记缺陷）

- 成员离线期间 create/invite/kick/leave 的 roster/通知不丢失：goutbox 补投，命令退出 0。
- unknown_group：成员未建群前收到群消息 → 对端回 ACK(reason=unknown_group)，
  发端条目保持 pending 等 roster 补投；若随后 roster 仍未达，条目持续 pending 并留告警。
- 身份互斥：同 data-dir 同时仅一个进程持有身份（chat serve / group send 均受 D6 约束）。
