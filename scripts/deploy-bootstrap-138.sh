#!/usr/bin/env bash
# 在公网服务器 138 上编译并部署 p2p-bootstrap（systemd 常驻）。
# 前置：138 已有 cargo（~/.cargo/bin）、rsproxy 镜像（~/.cargo/config.toml）、
#       ~/p2p-lab/{bin,data,logs} 目录、ufw 已放行 3400/udp 与 3401/tcp。
# 用法：scripts/deploy-bootstrap-138.sh [user@host]   # 默认 ops@43.240.223.138
set -euo pipefail

HOST="${1:-ops@43.240.223.138}"
REMOTE_SRC='~/src/p2p'
BIN_DIR='/home/ops/p2p-lab/bin'
DATA_DIR='/home/ops/p2p-lab/data'
LOG_DIR='/home/ops/p2p-lab/logs'
QUIC_PORT=3400
TCP_PORT=3401

log() { printf '[deploy] %s\n' "$*"; }

log "1/5 同步源码到 $HOST:$REMOTE_SRC（不含 .git/target/.worktrees/.env）"
ssh -o BatchMode=yes "$HOST" "mkdir -p $REMOTE_SRC"
rsync -a --delete \
  --exclude '.git' --exclude 'target' --exclude '.worktrees' \
  --exclude '.env' --exclude 'node_modules' \
  ./ "$HOST:$REMOTE_SRC/"

log "2/5 远端编译 p2p-cli（release）"
ssh -o BatchMode=yes "$HOST" \
  "export PATH=\$HOME/.cargo/bin:\$PATH && cd $REMOTE_SRC && cargo build --release -p p2p-cli"

log "3/5 安装二进制与目录"
ssh -o BatchMode=yes "$HOST" \
  "mkdir -p $BIN_DIR $DATA_DIR $LOG_DIR && \
   install -m 0755 $REMOTE_SRC/target/release/p2p-cli $BIN_DIR/p2p-cli && \
   $BIN_DIR/p2p-cli --version"

log "4/5 写入 systemd unit 并启动"
ssh -o BatchMode=yes "$HOST" "sudo -n tee /etc/systemd/system/p2p-bootstrap.service >/dev/null <<UNIT
[Unit]
Description=P2P bootstrap (rendezvous + relay)
After=network-online.target
Wants=network-online.target

[Service]
User=ops
ExecStart=$BIN_DIR/p2p-cli bootstrap --data $DATA_DIR --listen-quic 0.0.0.0:$QUIC_PORT --listen-tcp 0.0.0.0:$TCP_PORT --observation-port 3402
Restart=always
RestartSec=5
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
UNIT
sudo -n systemctl daemon-reload && \
sudo -n systemctl enable --now p2p-bootstrap && \
sleep 2"

log "5/5 健康检查"
ssh -o BatchMode=yes "$HOST" \
  "systemctl is-active p2p-bootstrap && \
   systemctl --no-pager -l status p2p-bootstrap | head -8 && \
   echo '--listening--' && ss -ulnp | grep $QUIC_PORT && ss -tlnp | grep $TCP_PORT"

log "完成。查看日志：ssh $HOST 'journalctl -u p2p-bootstrap -f'"
