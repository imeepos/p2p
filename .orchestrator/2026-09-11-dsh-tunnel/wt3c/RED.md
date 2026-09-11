# W-T3c 段①红证据（2026-09-11，修复前 @ feat/wt3c-origin-rewrite = e7229f7a + 测试先行）

命令：`cargo test --lib tunnel::`（apps/gui/src-tauri，crate p2p-console）
结果：**FAILED. 15 passed; 4 failed** —— 4 个失败全部是 Origin/Referer 重写断言（修复前必红）：

1. `tunnel::visit::head::tests::origin_referer_rewrite_to_target_authority`
   期望 Origin/Referer 重写为 `http://127.0.0.1:3080`，实测保持原头（未改写）。
2. `tunnel::visit::head::tests::ws_upgrade_origin_rewritten_too`
   升级请求 Origin 未重写（同上）。
3. `tunnel::visit::proxy::tests::post_origin_referer_rewritten_end_to_end`
   目标侧收到的报文原文（红证据核心，与 W-T3b 抓包一致）：
   ```
   POST /api/session HTTP/1.1
   Host: 127.0.0.1:59130            ← Host 已重写
   Origin: http://127.0.0.1:59132   ← 仍是反代端口，未重写 → DSH authority 栅栏 403 根因
   Referer: http://127.0.0.1:59132/?token=t
   Content-Type: application/json
   Content-Length: 2
   Connection: close
   ```
4. `tunnel::visit::proxy::tests::websocket_upgrade_pumps_raw_bytes_both_ways`
   （升级请求带 Origin 后）目标侧断言「升级请求 Origin 未重写」panic → 客户端 EOF before 101。

护栏（修复前即绿，防回归）：`origin_referer_left_alone_when_absent_or_foreign`
—— 无头不造头、`Origin: null` 与非回环 host（localhost）原样保留。
