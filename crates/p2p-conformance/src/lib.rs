//! 纯测试型 crate：全部断言逻辑在 tests/（panic-hygiene 门禁要求
//! src 非测试路径零 unwrap/expect/panic，故 lib 保持空壳）。
//!
//! 向量唯一源 docs/protocol/vectors/*.json；公共 helper 见 tests/common/mod.rs。
