#!/usr/bin/env python3
"""Final reproducible driver for the p2p GUI mockup set (gpt-image-2).

Pipeline (proven 2026-09-09): 01 board via /images/generations, then every
screen via /images/generations with the board tokens inlined in the prompt
plus an anti-board negative constraint. Serial execution, 15s-backoff
retries, optional low-quality draft fallback.

Usage:
  python3 generate.py                 # generate all missing images
  python3 generate.py --only 03-network
  python3 generate.py --refetch 05-messages   # delete then regenerate
  python3 generate.py --mode edits    # use board-reference edits channel
  python3 generate.py --fallback-low  # allow low-quality draft on failure

Reads OPENAI_API_KEY / OPENAI_BASE_URL from the nearest .env. The key is
passed to curl via a temp curlrc and never printed or written to logs.
"""
import argparse
import base64
import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

MODEL = "gpt-image-2"
SIZE = "1536x960"  # 16:10 desktop canvas, matches min window 960x600
QUALITY = "high"
RETRIES = 3
BACKOFF = 15
TIMEOUT = 240

ANCHOR = (
    "IMPORTANT: this is a real application screen, NOT a design-system "
    "board. Do NOT draw color swatches, typography samples, component "
    "galleries or labeled spec sections. Fill the whole window with the "
    "app UI described below. Style anchors: light theme, white background "
    "#FFFFFF, surfaces #F7F8FA and #EFF1F3, primary WeChat green #07C160, "
    "info blue #3B82F6, warning amber #F59E0B, error red #EF4444, corner "
    "radii 6/10/16, Tauri desktop window with 56px left icon rail (Chat, "
    "Contacts, Network, Messages, Docs, LLM Share, Agents, Settings pinned "
    "bottom), top bar with title 'p2p' and green 'Node Online' pill, bottom "
    "status bar 'Running / Port 47101 / 12 connections / v0.1.3'."
)


def find_env(start: Path) -> Path:
    for d in [start, *start.parents]:
        cand = d / ".env"
        if cand.is_file():
            return cand
    raise FileNotFoundError(".env not found")


def load_env() -> dict:
    env = {}
    for line in find_env(Path(__file__).resolve()).read_text().splitlines():
        line = line.strip()
        if line and not line.startswith("#") and "=" in line:
            k, v = line.split("=", 1)
            env[k.strip()] = v.strip().strip('"').strip("'")
    missing = [k for k in ("OPENAI_API_KEY", "OPENAI_BASE_URL") if not env.get(k)]
    if missing:
        raise RuntimeError("missing env keys: %s" % ",".join(missing))
    return env


def curl_post(env, url, args):
    """POST via curl with Authorization kept out of the process argv."""
    with tempfile.NamedTemporaryFile("w", suffix=".curlrc", delete=False) as cfg:
        cfg.write('header = "Authorization: Bearer %s"\n' % env["OPENAI_API_KEY"])
        cfg_path = cfg.name
    try:
        proc = subprocess.run(
            ["curl", "-sS", "--max-time", str(TIMEOUT),
             "-w", "\n%{http_code}", "--config", cfg_path] + args,
            capture_output=True, timeout=TIMEOUT + 30)
    finally:
        os.unlink(cfg_path)
    body, _, code = proc.stdout.rpartition(b"\n")
    return code.strip().decode(), body


def post_generations(env, prompt, quality):
    payload = {"model": MODEL, "prompt": prompt, "size": SIZE,
               "quality": quality, "n": 1}
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
        json.dump(payload, f)
        body_path = f.name
    try:
        return curl_post(env, None, [
            "-X", "POST", env["OPENAI_BASE_URL"].rstrip("/") + "/images/generations",
            "-H", "Content-Type: application/json",
            "--data-binary", "@" + body_path])
    finally:
        os.unlink(body_path)


def post_edits(env, prompt, board: Path, quality):
    return curl_post(env, None, [
        "-X", "POST", env["OPENAI_BASE_URL"].rstrip("/") + "/images/edits",
        "-F", "image=@%s;type=image/png" % board,
        "-F", "prompt=%s" % prompt,
        "-F", "model=%s" % MODEL,
        "-F", "size=%s" % SIZE,
        "-F", "quality=%s" % quality,
        "-F", "n=1"])


def extract_b64(body: bytes):
    try:
        d = json.loads(body)
        if d.get("data") and d["data"][0].get("b64_json"):
            return d["data"][0]["b64_json"]
        return None
    except Exception:
        for line in body.decode(errors="replace").splitlines():
            if line.startswith("data:"):
                chunk = line[5:].strip()
                if chunk and chunk != "[DONE]":
                    try:
                        o = json.loads(chunk)
                        if isinstance(o, dict) and o.get("b64_json"):
                            return o["b64_json"]
                    except Exception:
                        pass
    return None


def log_attempt(log_path: Path, rec: dict):
    records = []
    if log_path.exists():
        records = json.loads(log_path.read_text()).get("records", [])
    records.append(rec)
    log_path.write_text(json.dumps({"records": records}, indent=1))


def run_one(env, spec, out_dir: Path, mode: str, board: Path, allow_low: bool):
    dest = out_dir / spec["file"]
    if dest.exists():
        print("skip", spec["id"], "(exists)")
        return True
    for attempt in range(1, RETRIES + 1):
        t0 = time.time()
        if mode == "edits" and board.exists():
            status, body = post_edits(env, spec["prompt"], board, QUALITY)
        else:
            status, body = post_generations(
                env, spec["prompt"] + " " + ANCHOR, QUALITY)
        b64 = extract_b64(body) if status == "200" else None
        print(spec["id"], "try", attempt, "HTTP", status,
              "%.0fs" % (time.time() - t0), "ok" if b64 else "-", flush=True)
        log_attempt(out_dir / "tools" / "generate.log.json", {
            "id": spec["id"], "attempt": attempt, "status": status,
            "ok": bool(b64), "mode": mode,
            "secs": round(time.time() - t0)})
        if b64:
            dest.write_bytes(base64.b64decode(b64))
            return True
        time.sleep(BACKOFF)
    if allow_low:
        status, body = post_generations(env, spec["prompt"] + " " + ANCHOR, "low")
        b64 = extract_b64(body) if status == "200" else None
        if b64:
            dest.write_bytes(base64.b64decode(b64))
            print(spec["id"], "saved low-quality draft")
            return True
    print(spec["id"], "FAILED after retries; kept for a later run")
    return False


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--only", help="regenerate a single spec id")
    ap.add_argument("--refetch", help="delete existing file then regenerate")
    ap.add_argument("--mode", choices=["generations", "edits"],
                    default="generations")
    ap.add_argument("--fallback-low", action="store_true")
    args = ap.parse_args()

    out_dir = Path(__file__).resolve().parent.parent
    sys.path.insert(0, str(out_dir / "tools"))
    import specs  # noqa: E402

    env = load_env()
    board = out_dir / "01-design-system.png"
    chosen = [s for s in specs.SPECS
              if args.only is None or s["id"] == args.only]
    ok_all = True
    for spec in chosen:
        dest = out_dir / spec["file"]
        if args.refetch and spec["id"] in (args.refetch, args.only or ""):
            dest.unlink(missing_ok=True)
        ok = run_one(env, spec, out_dir, args.mode, board, args.fallback_low)
        ok_all = ok_all and ok
        time.sleep(3)
    print("DONE" if ok_all else "DONE-WITH-FAILURES")


if __name__ == "__main__":
    main()
