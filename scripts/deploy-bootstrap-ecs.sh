#!/usr/bin/env bash
# 在阿里云 ECS（第二公网节点）上部署 p2p-bootstrap（systemd 常驻）。
# 参照 deploy-bootstrap-138.sh，差异：
#   - 登录 root+密码：凭据只在 .env（SSH_HOST/SSH_USER/SSH_PASSWORD），经
#     SSH_ASKPASS 环境变量通道传递，不进命令行参数、不写任何文件（需 OpenSSH >= 8.4）；
#     worktree 内运行需指定 DEPLOY_ENV_FILE=<主树>/.env（.env 不入库，worktree 无副本）；
#   - 远端工具链从零就位：apt 装 build-essential/curl/rsync，rustup 走 rsproxy 镜像，
#     ~/.cargo/config.toml 配 rsproxy sparse 源；
#   - ufw 不启用：防火墙由阿里云安全组承担（已放行 22/3400udp/3401tcp/3402udp）。
# 端口与 138 对齐：QUIC 3400/udp + TCP 3401/tcp + 观测反射 3402/udp。
# 用法：scripts/deploy-bootstrap-ecs.sh   （幂等，可重复执行重部署）
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

QUIC_PORT=3400
TCP_PORT=3401
OBS_PORT=3402
REMOTE_SRC='/root/src/p2p'
LAB_DIR='/root/p2p-lab'
SSH_OPTS=(-o StrictHostKeyChecking=accept-new -o ConnectTimeout=15)
ASKPASS_FILE=''

log() { printf '[deploy-ecs] %s\n' "$*"; }

cleanup() { [ -z "$ASKPASS_FILE" ] || rm -f "$ASKPASS_FILE"; }
trap cleanup EXIT

# 凭据加载：.env -> 环境变量；缺失即失败，绝不内联密钥
# worktree 内主树 .env 不可见，用 DEPLOY_ENV_FILE=<主树>/.env 显式指定
load_credentials() {
  local env_file="${DEPLOY_ENV_FILE:-.env}"
  if [ -f "$env_file" ]; then set -a; . "$env_file"; set +a; fi
  : "${SSH_HOST:?SSH_HOST 未设置（检查仓库根 .env）}"
  : "${SSH_USER:?SSH_USER 未设置（检查 .env）}"
  : "${SSH_PASSWORD:?SSH_PASSWORD 未设置（检查 .env）}"
}

# 密码通道：askpass 辅助脚本只引用 $SSH_PASSWORD 环境变量，密码值本身不落盘
make_askpass() {
  ASKPASS_FILE="$(mktemp "${TMPDIR:-/tmp}/ecs-askpass.XXXXXX")"
  printf '#!/bin/sh\nprintf %%s "$SSH_PASSWORD"\n' > "$ASKPASS_FILE"
  chmod 700 "$ASKPASS_FILE"
  export SSH_ASKPASS="$ASKPASS_FILE" SSH_ASKPASS_REQUIRE=force
  export DISPLAY="${DISPLAY:-:0}"
}

remote() { ssh "${SSH_OPTS[@]}" "$SSH_USER@$SSH_HOST" "$@"; }

ensure_toolchain() {
  log "1/5 远端工具链就位（apt + rustup + rsproxy，已就位则跳过）"
  remote bash -s <<'EOS'
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
if ! command -v cc >/dev/null 2>&1 || ! command -v curl >/dev/null 2>&1 || ! command -v rsync >/dev/null 2>&1; then
  apt-get update -qq
  DEBIAN_FRONTEND=noninteractive apt-get install -y -qq build-essential curl rsync ca-certificates
fi
if ! command -v cargo >/dev/null 2>&1; then
  export RUSTUP_DIST_SERVER=https://rsproxy.cn
  export RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup
  # 安装脚本同样走 rsproxy 镜像：sh.rustup.rs 从大陆云机常被连接重置（curl 35）
  curl --proto '=https' --tlsv1.2 -sSf https://rsproxy.cn/rustup-init.sh \
    | sh -s -- -y --profile minimal
fi
mkdir -p "$HOME/.cargo"
if ! grep -q rsproxy "$HOME/.cargo/config.toml" 2>/dev/null; then
  cat > "$HOME/.cargo/config.toml" <<'TOML'
[source.crates-io]
replace-with = 'rsproxy-sparse'
[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"
[net]
git-fetch-with-cli = true
TOML
fi
command -v cargo >/dev/null 2>&1 || { echo "cargo 安装失败" >&2; exit 1; }
cargo --version
EOS
}

push_source() {
  log "2/5 同步源码到 $SSH_HOST:$REMOTE_SRC（不含 .git/target/.worktrees/.env）"
  remote "mkdir -p $REMOTE_SRC"
  rsync -a --delete -e "ssh ${SSH_OPTS[*]}" \
    --exclude '.git' --exclude 'target' --exclude '.worktrees' \
    --exclude '.env' --exclude 'node_modules' --exclude '.devloop' \
    ./ "$SSH_USER@$SSH_HOST:$REMOTE_SRC/"
}

build_remote() {
  log "3/5 远端编译 p2p-cli（release，首次需数分钟）"
  remote "export PATH=\$HOME/.cargo/bin:\$PATH && cd $REMOTE_SRC && cargo build --release -p p2p-cli"
}

install_and_start() {
  log "4/5 安装二进制 + systemd 常驻（Restart=always）"
  remote env LAB_DIR="$LAB_DIR" REMOTE_SRC="$REMOTE_SRC" QUIC_PORT="$QUIC_PORT" \
    TCP_PORT="$TCP_PORT" OBS_PORT="$OBS_PORT" bash -s <<'EOS'
set -euo pipefail
mkdir -p "$LAB_DIR/bin" "$LAB_DIR/data" "$LAB_DIR/logs"
install -m 0755 "$REMOTE_SRC/target/release/p2p-cli" "$LAB_DIR/bin/p2p-cli"
"$LAB_DIR/bin/p2p-cli" --version
cat > /etc/systemd/system/p2p-bootstrap.service <<UNIT
[Unit]
Description=P2P bootstrap ECS (rendezvous + relay)
After=network-online.target
Wants=network-online.target

[Service]
User=root
ExecStart=$LAB_DIR/bin/p2p-cli bootstrap --data $LAB_DIR/data --listen-quic 0.0.0.0:$QUIC_PORT --listen-tcp 0.0.0.0:$TCP_PORT --observation-port $OBS_PORT
Restart=always
RestartSec=5
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
UNIT
systemctl daemon-reload
systemctl enable p2p-bootstrap
systemctl restart p2p-bootstrap
sleep 2
EOS
}

health_check() {
  log "5/5 健康检查（远端监听 + 本机 TCP 可达性）"
  remote "systemctl is-active p2p-bootstrap \
    && ss -ulnp | grep -E ':$QUIC_PORT |:$OBS_PORT ' \
    && ss -tlnp | grep ':$TCP_PORT '"
  if command -v nc >/dev/null 2>&1; then
    nc -z -w 5 "$SSH_HOST" "$TCP_PORT" \
      && log "ECS $SSH_HOST:$TCP_PORT 本机可达" \
      || { log "ECS TCP $TCP_PORT 本机不可达（检查安全组/网络）"; exit 1; }
  fi
  log "完成。查看日志：ssh $SSH_USER@$SSH_HOST 'journalctl -u p2p-bootstrap -f'"
}

load_credentials
make_askpass
ensure_toolchain
push_source
build_remote
install_and_start
health_check
