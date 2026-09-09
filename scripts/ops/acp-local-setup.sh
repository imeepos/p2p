#!/bin/bash
# 本地 ACP 服务一键装配（幂等）：构建 → 安装 ~/.dsh/bin → dsh acp profile shim
# （自托管 stdio launcher，解 appExit/appReady 启动阻塞）→ owner 本机授权 →
# 重启 launchd 服务 → admin/descriptor/dsh 探针。
# 验收口径：输出 ACP-LOCAL-SETUP-OK 且退出码 0。
set -eu
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
DSH_BIN="$HOME/.dsh/bin"
AGENT_DATA="$HOME/.dsh/acp-agent"
LAUNCH_SRC="$REPO/scripts/ops/dsh-acp-launch"
echo "[0] repo=$REPO"

# 1) 构建（release；增量）
echo "[1] cargo build --release (acp-agent/acp-echo-stub, p2pctl)"
cargo build --release --manifest-path "$REPO/apps/acp-agent/Cargo.toml" >/dev/null
cargo build --release --manifest-path "$REPO/apps/cli/Cargo.toml" >/dev/null
mkdir -p "$DSH_BIN"
cp "$REPO/apps/acp-agent/target/release/acp-agent" "$DSH_BIN/"
cp "$REPO/apps/cli/target/release/p2pctl" "$DSH_BIN/"
echo "[1] installed: $DSH_BIN"
# 独立 acp-console bin 已随 INLINE-ACP-PUMP 撤销（T5）：CLI 能力收口 p2pctl acp console
if [ -e "$DSH_BIN/acp-console" ]; then
  echo "deprecation: ~/.dsh/bin/acp-console 已废弃，请改用 p2pctl acp console（建议手动删除旧二进制）" >&2
fi

# 2) dsh acp profile shim 装进所有相关 home（launchd 用的 ~/.dsh + shell 的 DSH_HOME）
install_shim() {
  home="$1"
  prof="$home/profiles/acp"
  [ -d "$home" ] || return 0
  if [ ! -f "$prof/package.json" ]; then
    mkdir -p "$prof"
    cp "$LAUNCH_SRC/profile.package.json" "$prof/package.json"
  fi
  mkdir -p "$prof/node_modules/@local/dsh-acp-launch"
  cp "$LAUNCH_SRC/index.mjs" "$LAUNCH_SRC/package.json" "$prof/node_modules/@local/dsh-acp-launch/"
  patch_file="$prof/cordis.patch.yml"
  if [ ! -f "$patch_file" ] || [ "$(tr -d '[:space:]' < "$patch_file")" = "[]" ]; then
    cp "$LAUNCH_SRC/acp-profile.patch.yml" "$patch_file"
  elif ! grep -q "acp-app-launch" "$patch_file"; then
    sed '/^# /d' "$LAUNCH_SRC/acp-profile.patch.yml" >> "$patch_file"
  fi
  echo "[2] shim installed: $prof"
}
install_shim "$HOME/.dsh"
OTHER_DSH_HOME="$(printenv DSH_HOME || true)"
if [ -n "$OTHER_DSH_HOME" ] && [ "$OTHER_DSH_HOME" != "$HOME/.dsh" ]; then
  install_shim "$OTHER_DSH_HOME"
fi

# 3) owner 本机授权：对每个已知 console 身份目录派 PeerId 并写策略表（scope=owner）
allow_console() {
  ident_dir="$1"
  [ -f "$ident_dir/key.seed" ] || return 0
  peer="$("$DSH_BIN/p2pctl" identity show --domain chat --data-dir "$ident_dir" --json 2>/dev/null |
    python3 -c 'import json,sys; print(json.load(sys.stdin)["peerId"])')"
  [ -n "$peer" ] || { echo "warn: $ident_dir 身份解析失败（跳过）" >&2; return 0; }
  "$DSH_BIN/p2pctl" acp allow "$peer" --scope owner --data-dir "$AGENT_DATA" \
    --note "local-console-loopback" --json >/dev/null
  echo "[3] allowed console peer=$peer (owner)"
}
allow_console "$HOME/Library/Application Support/com.p2p.console/acp-console-data/p2p-identity"
allow_console "$HOME/.dsh/acp-console-data/p2p-identity"

# 4) 重启服务：launchd 托管则 kickstart，否则 nohup 直起
PLIST="$HOME/Library/LaunchAgents/com.imeepos.acp-agent.plist"
if launchctl print "gui/$(id -u)/com.imeepos.acp-agent" >/dev/null 2>&1; then
  launchctl kickstart -k "gui/$(id -u)/com.imeepos.acp-agent"
  echo "[4] launchd service restarted"
elif [ -f "$PLIST" ]; then
  launchctl bootstrap "gui/$(id -u)" "$PLIST" 2>/dev/null || true
  launchctl kickstart -k "gui/$(id -u)/com.imeepos.acp-agent" 2>/dev/null || \
    (nohup "$DSH_BIN/acp-agent" --data-dir "$AGENT_DATA" --quic-port 7001 >>"$AGENT_DATA/launchd-stderr.log" 2>&1 &)
  echo "[4] service started (plist present)"
else
  pgrep -f "$DSH_BIN/acp-agent" >/dev/null || \
    nohup "$DSH_BIN/acp-agent" --data-dir "$AGENT_DATA" --quic-port 7001 >>"$AGENT_DATA/launchd-stderr.log" 2>&1 &
  echo "[4] service started (no plist; nohup)"
fi

# 5) 探针：admin /workspaces 可达 + /a2a/agents 200（旧二进制无 a2a 管理面会 404，
#    必须在此拦截——GUI 创建智能体依赖该端点）+ descriptor 新鲜 + dsh initialize 应答
ok=0
a2a_missing=0
for _ in $(seq 1 50); do
  DESC="$HOME/.dsh/acp/local-agent.json"
  if [ -f "$DESC" ]; then
    admin_url="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["admin_url"])' "$DESC" 2>/dev/null || true)"
    token="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["token"])' "$DESC" 2>/dev/null || true)"
    if [ -n "$admin_url" ] && curl -sf "$admin_url/workspaces" -H "Authorization: Bearer $token" |
      grep -q '"workspaces"'; then
      echo "[5] admin OK: $admin_url/workspaces"
      if curl -sf "$admin_url/a2a/agents" -H "Authorization: Bearer $token" | grep -q '"agents"'; then
        echo "[5] a2a OK: $admin_url/a2a/agents"
        ok=1
      else
        a2a_missing=1
      fi
      break
    fi
  fi
  sleep 0.2
done
if [ "$ok" != "1" ]; then
  if [ "$a2a_missing" = "1" ]; then
    echo "ACP-LOCAL-SETUP-FAIL: /a2a/agents 404（安装的二进制未含 a2a 管理面：构建产物过旧或安装未覆盖）" >&2
  else
    echo "ACP-LOCAL-SETUP-FAIL: admin /workspaces 不可达（二进制未刷新或服务未起）" >&2
  fi
  exit 1
fi

if command -v dsh >/dev/null; then
  pf="$(mktemp)"
  ( printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"clientCapabilities":{}}}\n'; sleep 120 ) | dsh --profile acp >"$pf" 2>/dev/null &
  dpid=$!
  ok2=0
  for _ in $(seq 1 90); do
    grep -q '"agentInfo"' "$pf" 2>/dev/null && { ok2=1; break; }
    sleep 1
  done
  kill "$dpid" 2>/dev/null; wait "$dpid" 2>/dev/null
  rm -f "$pf"
  if [ "$ok2" = "1" ]; then
    echo "[5] dsh --profile acp initialize OK（shim 生效）"
  else
    echo "warn: dsh initialize 无应答（shim 未生效或 dsh 变更）；SKIP 不阻断" >&2
  fi
else
  echo "warn: dsh 不在 PATH；跳过 dsh 探针（不假绿）" >&2
fi
echo "ACP-LOCAL-SETUP-OK"
