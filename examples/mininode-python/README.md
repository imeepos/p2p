# mininode-python: 第三方最小 p2p-base 参考节点(INTEROP-IV1)

一个**完全不依赖本仓源码**、仅凭 docs/protocol/ 文档实现的 Python 3.9 最小节点,
用于实证规范可接入性。与本仓 p2pctl 节点真实互通:签名注册、查询、ping 往返。

## 能力面

- 身份:Ed25519 种子文件(裸 32B,0600)-> 公钥 -> PeerId = base58(SHA-256(公钥))
- 传输:QUIC(aioquic 1.2.0)+ TLS1.3 双向自签身份证书
  (私有扩展 OID 1.3.6.1.4.1.59015.1 内容 = 32B 公钥,SPKI 与扩展逐字节一致,
  ALPN = p2p-base/1,SNI 占位 p2p-base);对端 PeerId 从证书推导,拨号带期望校验
- 帧:无符号 LEB128 varint 长度前缀 + payload,单帧 <= 1 048 576
- 协议:/p2p-base/ping/1(发起探测 + 应答回声);
  /p2p-base/rendezvous/1(SignedFields 签名注册、namespace/精确查号查询,
  protobuf 手写编码)
- 传输路径决策与探针记录:FEASIBILITY.md

## 运行

```bash
python3 -m venv .venv && .venv/bin/pip install -r requirements.txt

# 准备一个 p2pctl 节点(任意形态,取其 peerId 与 QUIC 监听端口):
p2pctl node start --data-dir /tmp/n --json     # 输出 peerId 与 127.0.0.1/uPORT

# mininode 全流程:连接 -> 注册自身 -> 查询确认 -> ping 对端
.venv/bin/python mininode.py \
  --seed ./seed.bin \
  --listen 127.0.0.1:0 \
  --bootstrap 127.0.0.1/uPORT \
  --peer <节点PeerId> \
  --namespace iv1-demo \
  --ttl-secs 60
# 成功输出 REGISTER-OK / QUERY-OK / PING-OK / MININODE-OK,退出码 0
```

常用参数:`--only register|query|ping|all` 单步执行;`--advertise ip:port`
指定注册地址;`--query-peer <PeerId>` 精确查号(缺省查自己);`--help` 看全表。

## 一键互操作冒烟

```bash
bash scripts/interop/mininode-smoke.sh
# 末行 SMOKE-OK,退出码 0;构建 p2pctl、建 venv、拉起节点、跑 mininode、按 PID 清理
```

## 实现所依据的文档章节映射

| 模块 | 依据 |
|---|---|
| identity.py | quickstart.md §1(种子 0600);wire-format.md §4.1(PeerId) |
| certmgr.py | wire-format.md §4.2(自签证书、OID 1.3.6.1.4.1.59015.1、SPKI==扩展) |
| framing.py | wire-format.md §6(varint/帧上限);vectors/varint.json、frame.json |
| pbcodec.py | rendezvous.md §2(消息字段表);vectors/rendezvous-register.json |
| ping.py | specs/ping.md §2/§3(回声语义、一问一答) |
| rendezvous.py | specs/rendezvous.md §2-§4;node-lifecycle.md §1.2(签名三点、限速常量) |
| quic_node.py | wire-format.md §4.2/§4.4/§5(TLS 双向身份、期望 PeerId、QUIC 流) |
| mininode.py | quickstart.md §3B/§5(bootstrap 接入、请求-响应原语) |
| selftest.py | spec-charter.md §8(向量为唯一黄金源);docs/protocol/vectors/* |

## 已知限制

- rendezvous 链路帧前缀按实测实现为 u32 大端(与 wire-format.md §6 的 varint
  漂移),依据与证据见 SPEC-GAPS.md GAP-1。
- mininode 作为服务端应答 ping,但不提供 rendezvous 服务端角色;
  未实现 Extended 能力(中继/打洞/chunked/mDNS)。
- 身份校验在握手完成后、任何业务数据交换前执行(不用本地 TLS alert,
  原因见 FEASIBILITY.md §3);这与 GAP-4 的文档空白相关。
- 拨号不支持 TCP/Noise 兜底路径(QUIC 单路;决策记录 FEASIBILITY.md)。
