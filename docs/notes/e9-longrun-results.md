# E9 长稳复测与保活间隔裁定（T15，2026-09-03）

- 基线：main（含 T8 relay 指标面 + T13 relay pub 面收口）；观测节点 = 新增
  `p2p-cli metrics` 子命令（relay-only 观测中继，16 项 relay_ 前缀指标可 grep）。
- 拓扑：15(.15，本机) ping → 102(B)，bootstrap 138+ECS 双面，relay 兜底
  **192.168.0.15/u3403（.15 上的 metrics 观测中继，本轮新增）** + 138/u3403。
  B=7S4nYViRNZ67mEBPEaHNLvkmDXwMeJeNyqUmRae7AYnW（102，~/p2p-lab/data/e4-b，
  二进制为 0289b9c 时代旧版）；观测中继与 ping 客户端为新二进制。
- 运维红线遵守：全程无 pkill，节点以精确 PID 管理（B pid 204809，观测中继 pid 32031）。

## 一、时序脆弱项处理（段 A）

relay_stability.rs 四处真实 sleep 全部处置（保活语义与断言零变更）：

| 原行号 | 处置 |
|---|---|
| :222 600ms | 改为 svc.metrics().keepalive_failures_total 收敛等待（事件驱动，yield 有界自旋） |
| :240 1s | 缩至 400ms（>1.3× silence，探活持续刷新）+ 失败计数基线零增量双确认；测试序重排使计数可隔离归因 |
| :288 5ms 轮询 | sleep 改 yield_now 有界自旋（5 万次上界），零真实时钟依赖 |
| :174 80ms | 窗口缩至 idle=100ms/gap=20ms（比例 1:5 不变），span 覆盖 ~10×TTL |

说明：tokio workspace 依赖为 features=["full"]（不含 test-util），且 Cargo.toml
不在本单允许清单，故采用上述等价手段而非 start_paused 虚拟时钟。
文件 315→299 行守住红线；改造后 relay 全量 43 用例绿。

## 二、真实拓扑三连采样（段 B）

命令（观测中继先行启动，见 §四 runbook 增补）：

```sh
scripts/e4-ping-sample.sh --peer-id 7S4nYViRNZ67mEBPEaHNLvkmDXwMeJeNyqUmRae7AYnW \
  --runs 3 --wait 15 \
  --bootstrap 43.240.223.138/u3400 --bootstrap 121.196.193.177/u3400 \
  --relay 192.168.0.15/u3403 --relay 43.240.223.138/u3403 \
  --out /tmp/e9-sample.tsv
```

三连 TSV（2026-09-03T16:41-16:42Z，全部经观测中继电路）：

```
utc	round	path	per_hop	rtt	reason	verdict
2026-09-03T16:41:55Z	1	Direct(false)>Punch(false)>Relay(true)	Punch:5014ms,Relay:10ms|Direct:2 addr(s) tried,Punch:probes failed; falling back to relay circuit,Relay:circuit established	15.010811291s	-	OK
2026-09-03T16:42:10Z	2	Direct(false)>Punch(false)>Relay(true)	Punch:5004ms,Relay:14ms|Direct:2 addr(s) tried,Punch:probes failed; falling back to relay circuit,Relay:circuit established	15.014461208s	-	OK
2026-09-03T16:42:25Z	3	Direct(false)>Punch(false)>Relay(true)	Punch:5014ms,Relay:14ms|Direct:2 addr(s) tried,Punch:probes failed; falling back to relay circuit,Relay:circuit established	15.01053775s	-	OK
SAMPLE	PASS|all 3 rounds via Direct(false)>Punch(false)>Relay(true)
```

三连 PASS；中继电路建立 10-14ms（观测中继在 LAN，快于轮 33 公网中继的
~90ms）；rtt 15.01s 恒定（打洞 5s 固定预算主导），极差 4ms。

采样窗口内观测中继指标（grep 原文，节选）：

```
relay_circuits_active=3
relay_circuits_bridged=1
relay_circuits_issued_total=18
relay_circuits_expired_total=0
relay_circuits_recycled_total=13
relay_keepalive_failures_total=7
relay_bridged_bytes_total=0
relay_links_active=2
relay_controls_registered=2
relay_punch_forwarded_total=6
```

## 三、保活间隔裁定（段 C）：数据达标，不调参

keepalive_failures_total 的 7 次增量时间线全部为精确 45s 周期，peer 归属
**100% 为 B（旧版 0289b9c 二进制，swarm 无控制面保活帧）**；三个新二进制
ping 客户端（每轮一个临时身份）零失败。裁定依据：

1. 新二进制 10s 保活 + 45s server_silence = 4.5 倍窗口，三连采样期间新
   客户端零判死，不构成调参依据；
2. 失败全部来自版本偏斜（旧版客户端无保活帧，45s 静默必然触发），随 E9
   全节点换装自然消除；调大 silence 只会推迟清理、不解决偏斜；
3. 每次清退后 B 立即重连重注册（links_active 恢复），未影响三连 PASS。

**结论：保活间隔（10s）与 server_silence（45s）在采样数据下达标，不调参；
control.rs 零改动。** 版本偏斜清退现象已入本档，作为换装验收的对照基线。

## 四、runbook 增补（§8.7 metrics 观测节点采样法）

见 docs/ops/experiment-env.md §8.7：观测中继启动命令、作为 --relay 首选
串入真实拓扑、stdout 直接 grep 16 项 relay_ 指标；采样后按精确 PID 停止。

## 五、发现与遗留

1. **旧版客户端周期清退**：0289b9c 旧版 swarm 无控制面保活，遇新默认
   server_silence=45s 每 45s 被清一次并立即重连（本档时间线）。E9 换装后
   消除；混合拓扑过渡期会有该 churn（可观测、可自愈）。
2. **bridged_bytes_total 不含异常结束的桥**：Err 结束（连接重置/空闲拆桥）
   时 copy_bidirectional 的部分字节计数丢失，仅正常关桥落账（本轮三连的
   电路以连接重置结束，故计数为 0）。埋点盲区登记，候选后续轮在
   copy 循环内增量计数。
3. **旧版节点默认绑定 loopback**：不带 --listen-quic/--observation 时
   listen_addrs 恒 127.0.0.1、rendezvous 注册被拒（no routable addr），
   节点不可被发现——runbook §8.7 已写明启动参数要求。
