# SPEC-GAPS: mininode 互操作轮发现的文档缺口与漂移

登记人:IV1(Python 参考节点,纯文档依据实现)。
登记原则:只列"仅凭 docs/protocol/ 文档实现会翻车或无解"的条目;每条附
黑盒观测证据(不引用仓内源码)。按严重度排序。

## GAP-1(阻断级)rendezvous 链路帧前缀与 wire-format.md §6 不一致

- 文档说法:wire-format.md §6 规定流上全部帧 = 无符号 LEB128 varint 长度前缀
  + payload;rendezvous.md §2 只写"帧 payload = protobuf 消息",§8 写
  "帧缝(一帧一消息,长度前缀封装)",未另立前缀格式,即被读作沿用 §6 varint。
- 实际行为(黑盒抓包证据,见 fake_rz_server.py 截获本仓客户端线上字节):
  流首帧(协议 ID)确为 varint 帧 `16 2f70...2f31`;其后每条消息为
  **4 字节大端(u32be)长度前缀 + protobuf**,实测样例
  `00 00 00 cb | 0a c8 01 | <200B Register>`(203 = Request 信封全长)。
  服务端响应同用 u32be(本节点按此适配后注册/查询互通成功)。
- 文档实现的下场:按 §6 varint 发 Register,服务端链路立即报
  "frame size too big" 断开,注册/查询全不可用。
- 建议:rendezvous.md §2 明确"链路帧 = u32be(len)+protobuf;协议 ID 帧除外,
  仍走 wire-format.md §6 varint";或统一改回 varint(需协调裁决,涉及存量)。

## GAP-2(运行级)lan-only 模式连本地 bootstrap 也被跳过,且日志误导

- 文档说法:cli-guide.md §lan-only:"不拨公网 bootstrap、不连公网 relay、
  不上报 observation,仅保留局域网发现与直连"。未说明 bootstrap 含
  私网/回环地址时的行为。
- 实际行为(节点守护日志):lanOnly=true 且 bootstrap=127.0.0.1/uPORT 时,
  装配日志输出 `bootstrap empty, rendezvous wiring skipped`——bootstrap 并非
  空,是 lan-only 把 bootstrap 接线整体跳过;日志文案与事实不符。
- 下场:想在本机做 rendezvous 服务端/客户端联调的人无法理解为何不注册。
- 建议:lan-only 语义改为"跳过非私网 bootstrap"或文档明示
  "lan-only 连本地 bootstrap 也不拨";日志改为真实原因。

## GAP-3(语义级)ping 应答方关流方式导致发起方 write_eof 冲突

- 文档说法:ping.md §3 发起方"读回应 -> 关流";应答方"handler 返回(关流)"。
  未定义应答方关流的方向语义。
- 实际行为(互操作实测):应答方回完 pong 即同时结束收发两个方向(对本仓
  客户端无感)。但第三方发起方按文档顺序"比对后 write_eof"时,流已被对端
  RESET(aioquic 抛 cannot call write() after reset() 断言)。
- 建议:ping.md §3 补一句"应答方返回即整流关闭,发起方读到 pong 后不得
  再写 FIN/数据,直接关闭即可"。

## GAP-4(语义级)PeerMismatch 的断开时机与可观测面未定义

- 文档说法:wire-format.md §4.4 只说"不一致即终止连接,错误 PeerMismatch"。
- 实际约束(传输库实测):在 TLS 握手内抛 bad_certificate 类 alert,会卡死
  aioquic 客户端的 wait_connected(alert 发生在 UDP 回调路径,连接状态机不
  收口);可行做法是握手完成、任何业务数据交换前断言并显式断连。
- 建议:文档写明校验失败的断开点("在交换任何应用数据前")与建议行为
  (立即关连、上抛显式错误),避免实现者走 TLS alert 路线。

## GAP-5(文档级)节点自我注册的默认 namespace 未记载

- 实际行为(抓包):本仓节点作为 rendezvous 客户端向 bootstrap 注册自身时,
  使用固定 namespace `p2p-base`(proto 字段1 len=8 "7032702d62617365")。
- 影响:第三方想查"标准节点"时无从知道该 namespace;quickstart §3 的伪代码
  也未给出。
- 建议:quickstart/rendezvous.md 登记默认 namespace 常量 `p2p-base`。

## GAP-6(低危)rendezvous 链路读端无超时的行为未定义

- 观测:发送不足 4 字节长度前缀时,服务端链路停留在等前缀状态(实测 2s
  窗口内无响应也无断开)。rendezvous.md §3/§4 未定义半帧悬挂的超时语义
  (对端是否最终回收、多久)。
- 建议:补"半帧/半前缀悬挂的回收超时",或注明依赖空闲连接回收(120s)兜底。

## 复核确认非缺口的条目(避免后人重查)

- 证书私有扩展内容 = 裸 32 字节公钥(非 DER 包装):与本仓节点互通成功,
  文档表述准确。
- Register.peer_id = 裸 SHA-256 摘要、SignedFields 编码、Ed25519 签名覆盖域:
  黄金向量 rendezvous-register.json 逐字节一致,一次通过。
- varint 边界/溢出、帧 1MiB 上限、peer-id 推导:向量全过;ping 路径的
  varint 帧与本仓实现一致。
