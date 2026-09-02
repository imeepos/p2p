# E4 执行阶段采样结果（2026-09-02）

- 基线：main@0289b9c（relay 7b901ed + quic idle-timeout 6dfc551），全节点换装后采样
- 拓扑：15(本机 A) ↔ 102(B)，双 bootstrap 138+ECS，双 relay 138/ECS u3403
- 节点：coordinator PID 73821（EhUaawMP…，data /tmp/p2p-n0）；102 B PID 197205（7S4nYViR…，data ~/p2p-lab/data/e4-b）
- 留场清理：n15 PID 66359、ecsn2 PID 97586 按精确 PID 清除；linc 未触碰

三连采样 TSV（scripts/e4-ping-sample.sh，目标=B）：

```
utc	round	path	per_hop	rtt	reason	verdict
2026-09-02T13:48:17Z	1	Direct(false)>Punch(false)>Relay(true)	Punch:5107ms,Relay:94ms|Direct:2 addr(s) tried,Punch:probes failed; falling back to relay circuit,Relay:circuit established	15.270008375s	-	OK
2026-09-02T13:48:33Z	2	Direct(false)>Punch(false)>Relay(true)	Punch:5091ms,Relay:89ms|Direct:2 addr(s) tried,Punch:probes failed; falling back to relay circuit,Relay:circuit established	15.244985667s	-	OK
2026-09-02T13:48:48Z	3	Direct(false)>Punch(false)>Relay(true)	Punch:5098ms,Relay:97ms|Direct:2 addr(s) tried,Punch:probes failed; falling back to relay circuit,Relay:circuit established	15.265382334s	-	OK
SAMPLE	PASS|all 3 rounds via Direct(false)>Punch(false)>Relay(true)
```

结论：

- 三连一致 PASS，逐跳路径显示 Relay 兜底命中（直连 hairpin 拒、打洞探测 5s 预算耗尽、中继电路 ~90ms 建立）
- RTT 分布 15.245s–15.270s（极差 25ms），其中 ~5.1s 为打洞探测固定预算，中继电路与 echo 往返稳定
- 全程无 code=3 配额自锁、无控制流秒断（7b901ed 修复生效）；无 30ms 空闲自断（6dfc551 修复生效）
