# CC2 进度（GuiConfig 三字段扩展与装配消费）

- 2026-09-17 20:26 CP1 落盘：契约三字段（rdRequireApproval/rdFps/tunnelServeAllow）双镜像（gui/cli types.rs）+ serde 字段级缺省 + Default 补齐；config.rs 测试拆出 config/tests.rs（300 行红线）并扩展红绿双向断言；types roundtrip json 抽 sample_json 复用；5 个集成测试 fixture 补齐穷举字面量；gui-contract §3 三字段入表。证据：cargo check --lib 双包 0 错；cargo test --lib（gui）157 passed；cargo test --bin p2pctl types 7 passed（含新增 config_rd_tunnel_fields_default_and_roundtrip）。
