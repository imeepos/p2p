# 最小接入路径（quickstart）

状态：v1。本文用**语言中立伪代码**描述全部步骤，可直接映射到任意语言实现；
伪代码仅表达语义，不复制本仓库源码。字节级细节见
[wire-format.md](wire-format.md)，常量出处见各表标注与
[node-lifecycle.md](node-lifecycle.md) 第 4 节。

## 0. 总览

接入一个兼容节点共五步：

1. 创建节点身份（Ed25519 种子）并落盘
2. 配置并监听 QUIC/TCP 端口
3. 入网：局域网 mDNS 或 引导节点 rendezvous（二选一或都开）
4. 注册自定义协议 handler
5. 发起一次请求-响应收发

## 1. 创建节点身份并落盘

身份 = Ed25519 种子（32 字节随机数）。种子落盘后重启身份不变（出处
crates/p2p-identity/src/seed.rs：裸 32 字节文件，权限 0600，加载时收紧宽松权限，
长度不符即报错，不静默重建）。

    # 伪代码
    fn load_or_create_identity(path):
        if file_exists(path):
            enforce_mode(path, 0600)          # 宽松权限一律收紧
            seed = read_file(path)
            if len(seed) != 32: error("seed file must be 32 bytes")
        else:
            seed = random_bytes(32)           # 操作系统 CSPRNG
            write_file(path, seed, mode=0600) # 目录按需创建并设 0700
        keypair = ed25519_keypair_from_seed(seed)
        return keypair

    pubkey  = keypair.public_key()            # 32 字节
    peer_id = base58( sha256(pubkey) )        # PeerId 推导，见 wire-format.md 第 4 节

PeerId 是节点唯一公开标识，展示为 base58 字符串（43-44 字符）。

## 2. 监听端口与配置

节点同时监听 QUIC（UDP）与 TCP 两路；两路语义一致，对端任一路可达即可。
配置项语义对齐 facade NodeConfig（出处 crates/p2p/src/lib.rs:23-74）：

    # 伪代码
    config = NodeConfig {
        quic_port: 3400,        # 0 = 随机端口（库默认）；3400 为 CLI 部署约定
        tcp_port:  3401,        # 0 = 随机端口（库默认）；3401 为 CLI 部署约定
        data_dir:  "./p2p-data",# 身份目录，权限 0700；种子文件在其内 0600
        enable_mdns: true,      # 库默认 true；仅本机调试可关
        bootstrap: [],          # 引导节点地址列表，格式见第 3 步 B
        relay_addrs: [],        # 中继服务地址；非空才启用打洞+中继兜底
        advertised_addrs: [],   # 对外宣告地址；空则用监听地址
    }

    node = Node::build(config)    # 完成身份装载、双路监听、发现接线
    print(node.local_peer_id())

其他可配项（非必需）：观测反射口（bootstrap 角色才启用，学习自身公网映射地址）、
rendezvous 公共策略 public_only（拒收全不可路由地址注册）、静态对端登记文件。
全部默认值见 crates/p2p/src/lib.rs:55-74。

## 3. 入网

### 方式 A：局域网 mDNS（零配置）

enable_mdns=true 即自动开始通告与浏览，无需任何参数：

- 服务类型固定 `_p2pbase._udp.local.`；通告周期默认 5s，TTL 默认 15s（出处
  crates/p2p-discovery/src/mdns.rs:17,44-45）。TXT 记录携带 peer（base58 PeerId）
  与可选 quic/tcp 端口；对端以通告地址 + 端口还原传输地址集。
- 适用边界：同一二层局域网。跨子网、跨 NAT 一律不可达，跨网必须走方式 B。

    # 伪代码：消费发现事件
    for event in node.events():
        if event.type == "PeerDiscovered" and event.source == "mdns":
            print(event.peer_id, event.addrs)   # 地址已入地址簿，可直接拨

### 方式 B：引导节点（rendezvous，跨网）

向一个公网可达的引导节点签名注册自身地址，并周期查询他人地址：

    # 伪代码
    config.bootstrap = ["203.0.113.10/u3400"]   # 引导节点，格式 IP/u端口 或 IP/t端口

    # 节点启动后自动循环：
    #   注册：每 20s 重发一次（兼作控制链路保活），TTL 默认 60s
    #   查询：启动期 5s 一轮、共 2 轮，之后 30s 一轮（出处
    #         crates/p2p-discovery/src/rendezvous/client.rs:20,25-29）
    # 注册帧带 Ed25519 签名，覆盖 namespace/peer_id/地址集/TTL/签发时刻，
    # 服务端验签三要点见 node-lifecycle.md 第 1.2 节

查指定节点（精确查号）：

    addrs = node.query_peer("<对方 base58 PeerId>")   # 返回 ["ip/u端口", ...]
    node.add_peer_address(peer_id, addrs[0])          # 入地址簿即可拨

### 可选：中继与打洞

    config.relay_addrs = ["203.0.113.10/u3403"]      # 中继服务地址（QUIC）

配置后拨号自动获得完整降级链：直连 -> 打洞 -> 中继电路兜底（见
node-lifecycle.md 第 2 节）。不配置则降级链止于直连。

## 4. 注册自定义协议 handler

协议 ID 必须满足语法 `/段/段/数字版本`（详见 wire-format.md 第 7 节），
如 `/myapp/echo/1`。handler 收到的流**已剥掉协议 ID 首帧**，直接读业务帧：

    # 伪代码：handler 接口
    class EchoHandler:
        fn protocol() -> "/myapp/echo/1"
        fn handle(peer: PeerId, stream):
            # 流已由分发层完成协议 ID 握手；handler 拥有该流直到返回
            loop:
                payload = stream.read_frame()      # varint 长度前缀 + 定长数据
                if payload == EOF: break
                stream.write_frame(payload)        # echo 原样回写
            # 函数返回即关流

    node.handle_protocol(EchoHandler)   # 注册后立即生效；重复注册同 ID 为覆盖

对端拨入未注册的协议 ID 时，收端关流并上抛 UnsupportedProtocol，不做猜测降级。
内置探活协议 `/p2p-base/ping/1`（收一帧原样回一帧）由节点自动注入，
你的同名注册优先。

## 5. 完成一次请求-响应

一问一答原语：开流 -> 写协议 ID -> 写请求帧 -> 读一帧回应 -> 关流；
一个超时覆盖全程（出处 crates/p2p-protocol/src/request_response.rs:29-47）：

    # 伪代码：调用侧（最常用）
    resp = node.request(
        peer      = 目标 PeerId,
        protocol  = "/myapp/echo/1",
        payload   = utf8("hello"),
        timeout   = 10s,
    )
    assert resp == utf8("hello")

等价的裸流写法（需要多帧交互时）：

    stream = node.new_stream(peer, "/myapp/echo/1")  # 内部完成协议 ID 首帧并冲刷
    stream.write_frame(utf8("hello"))
    echo = stream.read_frame()
    stream.close()

请求 payload 上限 1 MiB（帧上限）；更大消息用 chunked 分帧
（wire-format.md 第 10 节，上限 64 MiB）。

## 6. 端到端最小示例

把第 1-5 步串起来（双节点）：

    # 节点甲（响应方）
    kp   = load_or_create_identity("./p2p-data/seed")
    node = Node::build(NodeConfig{ quic_port: 3400, tcp_port: 3401, enable_mdns: true })
    node.handle_protocol(EchoHandler)
    for event in node.events(): print(event)      # 观察互发现

    # 节点乙（发起方，同局域网或配置同一 bootstrap）
    kp   = load_or_create_identity("./p2p-data-b/seed")
    node = Node::build(NodeConfig{ enable_mdns: true })
    # ... mDNS 发现甲后：
    resp = node.request(甲.peer_id, "/myapp/echo/1", utf8("hello"), 10s)

跨网部署时：把乙的 config.bootstrap 指向公网引导节点，其余不变。

## 7. 常见错误与排查

| 现象 | 原因与处置 |
|---|---|
| UnsupportedProtocol | 对端未注册该协议 ID；确认双方 ID 逐字符一致（含版本段数字） |
| FrameTooLarge | 单帧超过 1 MiB；改用 chunked 分帧 |
| PeerMismatch | 拨号时期望 PeerId 与握手推导不一致；核实地址是否指向了别的节点 |
| Timeout(request) | 链路不通或对端 handler 卡住；先看事件流有无 PeerDiscovered/连接事件 |
| no known address | 地址簿无该 PeerId；先完成发现（mDNS/rendezvous/add_peer_address） |
| rendezvous not wired | 未配置 bootstrap 就调 query_peer；配置引导地址后重试 |
| mDNS 发现不到 | 跨子网/虚拟机 NAT 均不通，属 mDNS 边界；改走方式 B |
| 拒绝自拨 | 拨自己的 PeerId 会被拒绝（refusing to dial self） |

## 8. 下一步

- 字节级自检：按 wire-format.md 第 11 节的十六进制样例验证你的 varint 与帧编解码。
- 发现与重连调优：node-lifecycle.md。
- 协议登记与版本演进规则：builtin-and-versioning.md。
