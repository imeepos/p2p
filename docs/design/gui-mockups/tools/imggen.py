#!/usr/bin/env python3
"""gpt-image-2 transport for GUI mockup generation.

Reads OPENAI_API_KEY / OPENAI_BASE_URL from the nearest .env (walks up from
this file). The key value is never printed, logged, or written to any file:
it is passed to curl via a temp curlrc that is deleted immediately.
HTTP goes through curl subprocess because urllib was reset by peer on host.
"""
import json
import os
import subprocess
import tempfile
from pathlib import Path

MODEL = "gpt-image-2"
TIMEOUT = 600


class ApiError(RuntimeError):
    pass


def find_env(start: Path) -> Path:
    for d in [start, *start.parents]:
        cand = d / ".env"
        if cand.is_file():
            return cand
    raise FileNotFoundError(".env not found in any parent directory")


def load_env() -> dict:
    env = {}
    for line in find_env(Path(__file__).resolve()).read_text().splitlines():
        line = line.strip()
        if line and not line.startswith("#") and "=" in line:
            k, v = line.split("=", 1)
            env[k.strip()] = v.strip().strip('"').strip("'")
    for k in ("OPENAI_API_KEY", "OPENAI_BASE_URL"):
        if os.environ.get(k):
            env[k] = os.environ[k]
    missing = [k for k in ("OPENAI_API_KEY", "OPENAI_BASE_URL") if not env.get(k)]
    if missing:
        raise RuntimeError("missing env keys: %s" % ",".join(missing))
    return env


def _curl(args, headers):
    """Run curl with headers from a temp config file; return (code, body, stderr)."""
    with tempfile.NamedTemporaryFile("w", suffix=".curlrc", delete=False) as cfg:
        for k, v in headers.items():
            cfg.write('header = "%s: %s"\n' % (k, v))
        cfg_path = cfg.name
    try:
        proc = subprocess.run(
            ["curl", "-sS", "--max-time", str(TIMEOUT),
             "-w", "\n%{http_code}", "--config", cfg_path] + args,
            capture_output=True, timeout=TIMEOUT + 30)
    finally:
        os.unlink(cfg_path)
    body, _, code = proc.stdout.rpartition(b"\n")
    try:
        status = int(code.strip() or b"0")
    except ValueError:
        status = 0
    return status, body, proc.stderr.decode(errors="replace")


def _build_json(env, path, payload):
    base = env["OPENAI_BASE_URL"].rstrip("/")
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
        json.dump(payload, f)
        body_path = f.name
    args = ["-X", "POST", base + path, "--data-binary", "@" + body_path]
    return args, [body_path]


def _build_multipart(env, path, fields, files):
    """files: list of (field_name, local_path); fields: str-keyed form values."""
    base = env["OPENAI_BASE_URL"].rstrip("/")
    args = ["-X", "POST", base + path]
    for k, v in fields.items():
        args += ["-F", "%s=%s" % (k, v)]
    for field, p in files:
        args += ["-F", "%s=@%s;type=image/png" % (field, p)]
    return args, []


def start_curl(env, args, json_mode):
    """Start curl in background; return ((proc, curlrc_path), tmp_paths stay with caller)."""
    headers = {"Authorization": "Bearer " + env["OPENAI_API_KEY"]}
    if json_mode:
        headers["Content-Type"] = "application/json"
    else:
        headers["Expect"] = ""
    with tempfile.NamedTemporaryFile("w", suffix=".curlrc", delete=False) as cfg:
        for k, v in headers.items():
            cfg.write('header = "%s: %s"\n' % (k, v))
        cfg_path = cfg.name
    proc = subprocess.Popen(
        ["curl", "-sS", "-N", "--max-time", str(TIMEOUT),
         "-w", "\n%{http_code}", "--config", cfg_path] + args,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    return proc, cfg_path


def finish_curl(proc, cfg_path, tmp_paths):
    out, err = proc.communicate()
    os.unlink(cfg_path)
    for p in tmp_paths:
        if p and os.path.exists(p):
            os.unlink(p)
    body, _, code = out.rpartition(b"\n")
    try:
        status = int(code.strip() or b"0")
    except ValueError:
        status = 0
    return status, body, err.decode(errors="replace")


def kill_curl(handle, tmp_paths):
    proc, cfg_path = handle
    if proc.poll() is None:
        proc.kill()
        proc.communicate()
    if os.path.exists(cfg_path):
        os.unlink(cfg_path)
    for p in tmp_paths:
        if p and os.path.exists(p):
            os.unlink(p)


def post_json(env, path, payload):
    args, tmps = _build_json(env, path, payload)
    handle = start_curl(env, args, json_mode=True)
    return finish_curl(handle[0], handle[1], tmps)


def post_multipart(env, path, fields, files):
    args, tmps = _build_multipart(env, path, fields, files)
    handle = start_curl(env, args, json_mode=False)
    return finish_curl(handle[0], handle[1], tmps)


def decode_stream(body):
    """Parse SSE stream; return (image_ref, usage) from the completed event."""
    best = None
    for line in body.decode(errors="replace").splitlines():
        if not line.startswith("data:"):
            continue
        chunk = line[5:].strip()
        if not chunk or chunk == "[DONE]":
            continue
        try:
            obj = json.loads(chunk)
        except json.JSONDecodeError:
            continue
        if isinstance(obj, dict) and obj.get("b64_json"):
            best = obj
    if not best:
        raise RuntimeError("stream completed without image data; tail="
                           + body[-200:].decode(errors="replace"))
    ref = ("b64", best["b64_json"]) if best.get("b64_json") else ("url", best.get("url"))
    return ref, best.get("usage", {})


def decode_response(body):
    """Return (image_ref, usage); image_ref is ('b64', str) or ('url', str)."""
    try:
        data = json.loads(body)
    except json.JSONDecodeError:
        snippet = body[:300].decode(errors="replace")
        raise RuntimeError("non-JSON response: " + snippet)
    if isinstance(data, dict) and data.get("error"):
        raise ApiError(json.dumps(data["error"])[:500])
    items = data.get("data") if isinstance(data, dict) else None
    if not items:
        raise RuntimeError("empty data array: " + body[:300].decode(errors="replace"))
    first = items[0]
    if first.get("b64_json"):
        return ("b64", first["b64_json"]), data.get("usage", {})
    if first.get("url"):
        return ("url", first["url"]), data.get("usage", {})
    raise RuntimeError("no b64_json/url in item keys=%s" % ",".join(first.keys()))


def fetch_url(env, url, out_path):
    headers = {"Authorization": "Bearer " + env["OPENAI_API_KEY"]}
    status, body, err = _curl(["-L", "-o", str(out_path), url], headers)
    if status != 200:
        raise RuntimeError("download failed http=%s %s %s" % (status, err[:200], body[:200]))


def postprocess_png(native_path, out_path, target_w, target_h):
    """Upscale LANCZOS + mild unsharp, then center-crop to the 16:9 target."""
    from PIL import Image, ImageFilter
    img = Image.open(native_path).convert("RGB")
    scale = target_w / img.width
    up = img.resize((target_w, round(img.height * scale)), Image.LANCZOS)
    up = up.filter(ImageFilter.UnsharpMask(radius=2, percent=80, threshold=2))
    top = (up.height - target_h) // 2
    out = up.crop((0, top, target_w, top + target_h))
    out.save(out_path, "PNG")
    return out.size


def save_b64(b64, out_path):
    import base64
    out_path = Path(out_path)
    out_path.write_bytes(base64.b64decode(b64))
    return out_path


def png_dimensions(path):
    data = Path(path).read_bytes()[:33]
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise RuntimeError("not a PNG file: %s" % path)
    w, h = struct_png(data)
    return w, h


def struct_png(data):
    import struct
    return struct.unpack(">II", data[16:24])
