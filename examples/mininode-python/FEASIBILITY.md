# FEASIBILITY: mininode-python 传输路径决策

状态:决策定稿(2026-09-09,INTEROP 轮 IV1)。结论:**首选 QUIC/aioquic 路径,TCP/Noise 兜底不启用**。

## 1. 决策

mininode 采用 QUIC 单路实现,协议栈对照 wire-format.md 第 1 节分层模型:

| 层 | 选择 | 依据 |
|---|---|---|
| 传输 | QUIC(aioquic 1.2.0) | 文档定义 QUIC 优先路径;单一 UDP 端口同时承载流复用 |
| 安全 | TLS1.3 自签身份证书 + 私有扩展 OID 1.3.6.1.4.1.59015.1 | wire-format.md 4.2;双向认证、SPKI 与扩展逐字节一致 |
| 复用 | QUIC 原生双向流 | wire-format.md 第 5 节;免实现 yamux |
| 帧语义 | varint 长度前缀 + payload | wire-format.md 第 6 节,与传输路径无关 |

理由:

1. 协议地位对等:TLS1.3 双向身份证书语义在文档中完整可循(证书扩展、ALPN、
   SNI 占位符、校验规则、PeerId 推导),无需任何源码知识即可实现。
2. 实现量:aioquic 提供 QUIC+TLS1.3 全栈;yamux 兜底路径需手写复用层
   (帧型、流状态机、背压),文档仅给参数串与记录帧格式,复用层语义需自行
   对齐 libp2p-yamux 通用知识,证明成本更高。
3. 探针实证:同进程回环 aioquic server/client 双端自证通过(见下)。

## 2. 探针设计与结果

探针脚本 `probe_quic_mtls.py`(纯文档依据实现,回环双端),验证四件事:

1. 自签 Ed25519 身份证书(扩展内容=原始 32 字节公钥)在 TLS1.3 握手可用,
   对端证书可取回并按 OID/SPKI 规则校验、推导 PeerId —— PASS;
2. ALPN 固定 `p2p-base/1` 协商成功,SNI 固定占位符 `p2p-base` —— PASS;
3. 帧流(协议 ID 帧 + ping 帧 + 回声帧)在 QUIC 双向流上往返一致 —— PASS;
4. 身份锚定:期望 PeerId 不符时连接在交换任何业务数据前被显式拒绝 —— PASS。

运行结果(本次实测):

```
probe-client: server identity anchored: <base58 PeerId>
probe-client: ping roundtrip OK over QUIC+identity TLS
probe-negative: connection rejected before any data: IdentityCertError(...)
PROBE-PASS
```

## 3. aioquic 能力边界实测记录

- 客户端证书(mTLS):aioquic 客户端在服务端发出 CertificateRequest 时,若
  `QuicConfiguration.certificate/private_key` 已配置,会发送 Certificate +
  CertificateVerify —— 能力在库中存在,客户端侧无需打补丁。
- 服务端请求客户端证书:aioquic 服务端仅经私有标志
  `tls.Context._request_client_certificate` 开启(库注释"for test purposes
  only")。mininode 自身监听面默认不强制客户端证书(仅 QUIC 路径语义),
  与本仓节点的互操作方向(mininode 拨号 -> p2pctl 监听)不受影响。
- 本地侧 TLS Alert 的已知行为:若在校验钩子中抛出 AlertBadCertificate,
  aioquic 1.2.0 的 wait_connected 会悬挂(异常发生在 UDP 回调路径,连接
  状态机未收口,实测复现)。因此"拒绝即断"实现为:**握手完成后、任何业务
  数据交换前**做身份断言,失败立即关连接并上抛显式错误。这保持
  wire-format.md 4.4 的语义(PeerMismatch 连接终止),避开库的悬挂路径。
- CA 系校验置 `verify_mode=CERT_NONE`:协议身份锚在证书扩展而非 CA 链,
  与文档"只认带此扩展的自签证书,不查 CA"一致。

## 4. 剩余互通风险与对策

| 风险 | 对策 |
|---|---|
| rustls(本仓节点)与 aioquic 的 TLS1.3 参数交集 | 探针双端已证 aioquic 侧自洽;rustls 侧交由真实冒烟脚本验证 |
| 证书扩展内容形态(裸 32B vs DER 包装) | 文档写明"内容为原始 32 字节公钥",按裸字节实现;冒烟失败则登记 SPEC-GAPS 复核 |
| ALPN/版本/密码套件协商 | 固定单值 `p2p-base/1`、QUIC v1、TLS1.3 默认套件 |
| rendezvous/ping 应用层语义 | 按 specs 页逐字实现,黄金向量已逐字节通过(selftest.py) |

## 5. 兜底路径(未启用,留档)

TCP/Noise XX 路径在 wire-format.md 4.3 有完整字节级描述(参数串、身份负载
96 字节、域串 `p2p-noise-xx-v1`、u16be 记录帧),向量 handshake-identity.json
可离线验证。若 QUIC 冒烟被证伪,按原卡降级方案启用;本轮未触发。
