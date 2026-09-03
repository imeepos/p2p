# 构建门禁（AGENTS.md 红线机械化）：make check 一键全跑
# cargo 预期在 $HOME/.cargo/bin，由各脚本自行处理 PATH
SHELL := /bin/bash
.DEFAULT_GOAL := check

.PHONY: check fmt fmt-check line-limit clippy test gui-check version-check gate-tests release-check

# 聚合门禁：先验证门禁脚本，再跑版本/格式/行数/clippy/测试/GUI
check: gate-tests version-check fmt-check line-limit clippy test gui-check

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

# GUI 门禁：lint + build + vitest（含启动冒烟，白屏事故后加入）
gui-check:
	bash scripts/check/gui.sh

# 版本一致性：apps/gui 三处版本（package.json / tauri.conf.json / Cargo.toml）必须同值
version-check:
	bash scripts/check/version.sh

# 门禁脚本自身的成功/失败路径回归，防止门禁实现退化为假绿
gate-tests:
	bash scripts/check/tests/release-gates.sh

# 发布总门禁：main 分支 + 干净工作树 + 版本一致 + make check（打 client-v tag 前在主树跑）
# 分支项可用 RELEASE_ALLOW_BRANCH=1 绕过（CI/测试环境不在 main 上时）
release-check:
	@if [ "$(RELEASE_ALLOW_BRANCH)" != "1" ] && [ "$$(git branch --show-current)" != "main" ]; then 		echo "release-check: FAIL 当前分支不是 main（CI/测试绕过：RELEASE_ALLOW_BRANCH=1）" >&2; exit 1; fi
	@if [ -n "$$(git status --porcelain)" ]; then 		echo "release-check: FAIL 工作树不干净（git status --porcelain 非空）" >&2; exit 1; fi
	@bash scripts/check/version.sh
	@$(MAKE) check
