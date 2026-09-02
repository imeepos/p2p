# 构建门禁（AGENTS.md 红线机械化）：make check 一键全跑
# cargo 预期在 $HOME/.cargo/bin，由各脚本自行处理 PATH
SHELL := /bin/bash
.DEFAULT_GOAL := check

.PHONY: check fmt fmt-check line-limit clippy test

# 聚合门禁：快失败在前（格式 -> 行数 -> clippy -> 测试）
check: fmt-check line-limit clippy test

# 自动修复格式
fmt:
	export PATH="$$HOME/.cargo/bin:$$PATH"; cargo fmt

# 格式检查（只读）
fmt-check:
	bash scripts/check/fmt.sh

# 单文件行数红线（默认 300，豁免见脚本头注释）
line-limit:
	bash scripts/check/line-limit.sh

# clippy 全 workspace，警告一律当错误
clippy:
	bash scripts/check/clippy.sh

# 全量测试
test:
	bash scripts/check/test.sh
