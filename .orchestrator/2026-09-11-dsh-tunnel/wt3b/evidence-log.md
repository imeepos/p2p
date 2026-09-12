# W-T3b 证据日志（带时间戳，按证据链项落盘）

来源：本轮执行 transcript 摘录；本地时区 America/Los_Angeles。原始命令输出全文见会话记录与 EVIDENCE.md。

## [06:24:33] DSH 隔离实例启动
```
DSH_HOME=/tmp/wt3b-dsh-home dsh web --no-open
→ dsh web: http://127.0.0.1:3080/?token=IXXs8L9aKgOcsCEZUl-NB6Ku1ZRCLnQLKcPeV-myiLw
```

## [06:29:34] 双 GUI 实例并行启动
```
P2P_CONTROL_PORT=31780 p2p-console  (A)
P2P_CONTROL_PORT=31781 p2p-console  (B)
→ GET /health 31780: {"pid":99580,"version":"0.1.7","route":"dashboard"} ok:true
→ GET /health 31781: {"pid":99581,...} ok:true
```

## [06:38:47] 证据#1 A 侧节点身份隔离 + serve 开启
```
page/action settings.saveAndRestart(dataDir=/tmp/wt3b-node-a) → peerId 5VBVBUpkzqKCtJ3koFabx1wWSBmuEWk9FjSqq3SVWZkY, running:true
driver invoke tunnel_serve_start {target:"127.0.0.1:3080"}
→ {"enabled":true,"allow":["127.0.0.1:3080"],"activeSessions":0}
driver invoke tunnel_status
→ {"active":false,...,"serve":{"enabled":true,"allow":["127.0.0.1:3080"]}}
```

## [06:38:57] 证据#2 B 侧节点隔离
```
settings.saveAndRestart(dataDir=/tmp/wt3b-node-b) → peerId 2VMiV9DWCXPKXmEUXBqhh3DE7zRYAYYjdJzjNg6xB7iU
node_status: 31780 peer 5VBV… dir /tmp/wt3b-node-a; 31781 peer 2VMi… dir /tmp/wt3b-node-b
```

## [07:05:44] 证据#2 B 侧真实表单开隧道
```
driver fill_open {url:"http://127.0.0.1:3080/?token=IXXs…", peer:"5VBV…"}
→ {"clicked":"开启远程访问"}   （#tunnel-dsh-url/#tunnel-peer 填入后点击真实按钮）
driver invoke tunnel_status (B)
→ {"active":true,"localAddr":"127.0.0.1:52172","target":"127.0.0.1:3080",
   "sessions":[{"sessionId":"a1e35abb048532be","peerId":"5VBV…","outcome":"open",...}]}
```

## [07:06:45–07:11:14] 证据#3/#4 浏览器经隧道访问 + 403 判别
```
browser → http://127.0.0.1:52172/?token=IXXs…
→ HTML 200（title=DeepSeek Harness，插件清单完整；4332962B vendor chunk 经隧道字节面通过）
→ POST /api/* 全部 403（响应体=DSH 原文 "forbidden"）
→ ws://127.0.0.1:52172/api/remote.mux 握手 403（Unexpected response code: 403）
对照：browser → http://127.0.0.1:3080/?token=IXXs…（直连）
→ POST /api/settings/describe 等 全部 200 OK
判别：GET 过隧道、POST/WS 不过 → Origin/Referer 未重写，DSH 按 authority 校验拒绝
```

## [07:15:54] 证据#6 回环证明 + 证据#7 负例
```
lsof -p <B> | LISTEN:
  localhost:31781 (控制通道) / localhost:52057 / localhost:52058 / localhost:52172 (反代)
  —— 反代无 *: 绑定 ✓（观察点：节点 swarm 自身 *:52056/*:52080 通配，非隧道域）
curl "http://127.0.0.1:3080/?token=BADtoken123"   → 401
curl "http://127.0.0.1:52172/?token=BADtoken123"  → 401（隧道透传 DSH 401）
```

## [07:15:16] 证据#8 审计行
```
tunnel_status.sessions：80+ 条八字段记录
sessionId/peerId/target/startedAt/endedAt/bytesIn/bytesOut/outcome
真实字节样例：bytesIn=4332962、191898、18478、8671…
终态闭集实测：ok / busy / io / open（open 为活动连接，endedAt=null）
```

## [07:16:48] 证据#7 GUI 错误三态（driver DOM 文本）
```
fill_open {url:"http://0.0.0.0:1/?token=bad"} → 点击「开启远程访问」
视图文本：badge「错误」+「最近错误 host 只允许 127.0.0.1 回环字面量: 0.0.0.0」
        +「请先在网络页启动本机 p2p 节点…」；「已开启」卡并存（本地地址/浏览器入口/目标/复制链接）
观察点：控制通道 /screenshot 三次捕获字节数全等(41758)，帧源滞后；DOM 文本取证为准
```

## [07:19:00] 收尾
```
pkill p2p-console ×2 / dsh web / vite 5174 / driver-proxy → 全部退出
gui-config.json ← /tmp/wt3b-gui-config-backup.json（CONFIG-RESTORED）
保留：/tmp/wt3b-dsh-home、/tmp/wt3b-node-a、/tmp/wt3b-node-b（复跑用）
```
