# 构建门禁（AGENTS.md 红线机械化）：make check 一键全跑
# cargo 预期在 $HOME/.cargo/bin，由各脚本自行处理 PATH
SHELL := /bin/bash
.DEFAULT_GOAL := check

.PHONY: check check-fast fmt fmt-check line-limit clippy test gui-check gui-tauri-check version-check gate-tests panic-hygiene protocol-registry cli-parity ai-docs-sync release-check

# 聚合门禁：先验证门禁脚本，再跑版本/格式/行数/clippy/测试/GUI/panic 卫生/协议注册表
check: gate-tests version-check fmt-check line-limit clippy test gui-check gui-tauri-check panic-hygiene protocol-registry cli-parity ai-docs-sync

# 分层快门禁（日常迭代）：便宜门禁恒跑，clippy/test/gui/tauri 按受影响域裁剪
# （scripts/check/affected.sh 判定：git 变更集 → crate 反向依赖闭包）。
# 只裁剪"该跑什么"，不伪造绿；合并进 main 前 / release-check / CI 仍用全量 check
check-fast:
	bash scripts/check/fast.sh

# 自动修复格式
fmt:
	export PATH="$$HOME/.cargo/bin:$$PATH"; cargo fmt

# 格式检查（只读）：根 workspace + src-tauri 独立 workspace（见 fmt.sh 头注释）
fmt-check:
	bash scripts/check/fmt.sh

# 单文件行数红线（默认 300，豁免见脚本头注释）
line-limit:
	bash scripts/check/line-limit.sh

# clippy 全 workspace + src-tauri 独立 workspace，警告一律当错误（ubuntu SKIP 口径见 clippy.sh）
clippy:
	bash scripts/check/clippy.sh

# 全量测试
test:
	bash scripts/check/test.sh

# GUI 门禁：lint + build + vitest（含启动冒烟，白屏事故后加入）
gui-check:
	bash scripts/check/gui.sh

# src-tauri 独立门禁：该 crate 脱离根 workspace，根 test/clippy 不覆盖；
# 非 macOS 且缺 webkit2gtk 时显式 SKIP（CI ubuntu），本机 macOS 真跑 cargo test
gui-tauri-check:
	bash scripts/check/gui-tauri.sh

# UI 回归（opt-in，不进 check）：macOS GUI 会话专用，分钟级：起 GUI 实例逐页
# navigate/descriptor/动作断言/截图；GUI 改动合并前后手动跑（分层门禁实践：
# 快门禁进 CI，重 E2E 独立靶）
ui-regression:
	bash scripts/ops/ui-regression.sh

# 版本一致性：apps/gui 三处版本（package.json / tauri.conf.json / Cargo.toml）必须同值
version-check:
	bash scripts/check/version.sh

# 门禁脚本自身的成功/失败路径回归，防止门禁实现退化为假绿
gate-tests:
	bash scripts/check/tests/ascii-var-guard.sh
	bash scripts/check/tests/release-gates.sh
	bash scripts/check/tests/panic-hygiene.sh
	bash scripts/check/tests/protocol-registry.sh
	bash scripts/check/tests/cli-parity.sh
	bash scripts/check/tests/mock-ipc-guards.sh
	bash scripts/check/tests/src-tauri-gate.sh
	bash scripts/check/tests/make-latest-json.sh
	bash scripts/check/tests/affected-fast.sh

# CLI 对等守卫：GUI generate_handler 全集 ↔ p2pctl 实测命令面（映射表 cli-parity.tsv）
cli-parity:
	bash scripts/check/cli-parity.sh

# AI 文档防漂移守卫：p2pctl 实测命令面 ↔ docs/ops/p2pctl-ai-guide.md（N1）
ai-docs-sync:
	bash scripts/check/ai-docs-sync.sh

# panic 卫生门禁：范围 crate 非测试路径 unwrap/expect/panic 清零（豁免清单见脚本同目录）
panic-hygiene:
	bash scripts/check/panic-hygiene.sh

# 协议注册表门禁：代码字面量 <-> registry.toml <-> wire-protocol.md 四向机械核对
protocol-registry:
	bash scripts/check/protocol-registry.sh

# 发布总门禁：main 分支 + 干净工作树 + 版本一致 + make check（打 client-v tag 前在主树跑）
# 分支项可用 RELEASE_ALLOW_BRANCH=1 绕过（CI/测试环境不在 main 上时）
release-check:
	@if [ "$(RELEASE_ALLOW_BRANCH)" != "1" ] && [ "$$(git branch --show-current)" != "main" ]; then 		echo "release-check: FAIL 当前分支不是 main（CI/测试绕过：RELEASE_ALLOW_BRANCH=1）" >&2; exit 1; fi
	@if [ -n "$$(git status --porcelain)" ]; then 		echo "release-check: FAIL 工作树不干净（git status --porcelain 非空）" >&2; exit 1; fi
	@bash scripts/check/version.sh
	@$(MAKE) check