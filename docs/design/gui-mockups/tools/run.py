#!/usr/bin/env python3
"""Generate the GUI mockup set with gpt-image-2.

Upstream latency is bimodal (39s..126s+) behind a ~126s Cloudflare wall, so
each attempt is a RACE: --race identical requests in flight, first 200 wins,
losers are killed. Waves continue until --max-attempts is exhausted.

Usage:
  python3 run.py [--only 02-workbench] [--race 3] [--max-attempts 2]

Writes PNGs into docs/design/gui-mockups/ and outcome records (endpoint,
params, attempts, usage) into tools/results.json. The API key is never
printed or persisted.
"""
import argparse
import json
import time
from pathlib import Path

import imggen
from specs import SPECS

TOOLS_DIR = Path(__file__).resolve().parent
OUT_DIR = TOOLS_DIR.parent
RESULTS = TOOLS_DIR / "results.json"
FALLBACK_PARAMS = ("stream", "partial_images", "thinking", "seed", "quality")
RACE_POLL_S = 1.0


def build_call(spec, params, env):
    """Return (json_mode, curl_args, tmp_paths) for one spec attempt."""
    payload = {"model": imggen.MODEL, "prompt": spec["prompt"]}
    payload.update({k: v for k, v in params.items() if v is not None})
    if spec["mode"] == "generations":
        args, tmps = imggen._build_json(env, "/images/generations", payload)
        return True, args, tmps
    files = [("image[]", str(OUT_DIR / ref)) for ref in spec.get("refs", [])]
    fields = {k: (str(v).lower() if isinstance(v, bool) else str(v))
              for k, v in payload.items()}
    args, tmps = imggen._build_multipart(env, "/images/edits", fields, files)
    return False, args, tmps


def race_call(spec, params, env, race):
    """Fire `race` identical curls; return first-200 result or the last failure."""
    pending = []
    for _ in range(race):
        json_mode, args, tmps = build_call(spec, params, env)
        handle = imggen.start_curl(env, args, json_mode=json_mode)
        pending.append((handle, tmps))
    last = (0, b"", "race never settled")
    while pending:
        ready = [e for e in pending if e[0][0].poll() is not None]
        if not ready:
            time.sleep(RACE_POLL_S)
            continue
        entry = ready[0]
        pending.remove(entry)
        (proc, cfg), tmps = entry
        status, body, err = imggen.finish_curl(proc, cfg, tmps)
        if status == 200:
            for loser_handle, loser_tmps in pending:
                imggen.kill_curl(loser_handle, loser_tmps)
            return status, body, err
        last = (status, body, err)
        print("[race] status=%s, %d still in flight" % (status, len(pending)),
              flush=True)
    return last


def materialize(spec, image_ref, env):
    native = TOOLS_DIR / "native" / spec["file"]
    native.parent.mkdir(exist_ok=True)
    final = OUT_DIR / spec["file"]
    kind, value = image_ref
    if kind == "b64":
        imggen.save_b64(value, native)
    else:
        imggen.fetch_url(env, value, native)
    w, h = imggen.png_dimensions(native)
    expected = spec["params"]["size"].split("x")
    if [w, h] != [int(x) for x in expected]:
        raise RuntimeError("dimension mismatch %sx%s != %s"
                           % (w, h, spec["params"]["size"]))
    tw, th = [int(x) for x in spec["target"].split("x")]
    fw, fh = imggen.postprocess_png(native, final, tw, th)
    if [fw, fh] != [tw, th]:
        raise RuntimeError("postprocess produced %dx%d" % (fw, fh))
    return {"native": str(native), "generated_size": "%dx%d" % (w, h),
            "delivered": "%dx%d" % (fw, fh),
            "postprocess": "PIL LANCZOS + unsharp + center crop",
            "path": str(final)}


def attempt_once(spec, params, env, race):
    status, body, err = race_call(spec, params, env, race)
    if status != 200:
        message = body[:400].decode(errors="replace") or err[:400]
        raise imggen.ApiError("http %s: %s" % (status, message))
    try:
        image_ref, usage = imggen.decode_stream(body)
    except RuntimeError:
        image_ref, usage = imggen.decode_response(body)
    dims = materialize(spec, image_ref, env)
    return {"usage": usage, **dims}


def run_spec(spec, env, max_attempts, race):
    params = dict(spec["params"])
    endpoint = ("/images/generations" if spec["mode"] == "generations"
                else "/images/edits")
    record = {"id": spec["id"], "file": spec["file"], "endpoint": endpoint,
              "mode": spec["mode"], "refs": spec.get("refs", []),
              "prompt": spec["prompt"], "params": params,
              "race": race, "attempts": [], "ok": False}
    for i in range(max_attempts):
        t0 = time.time()
        print("[run] %s wave %d/%d race=%d started" %
              (spec["id"], i + 1, max_attempts, race), flush=True)
        try:
            outcome = attempt_once(spec, params, env, race)
        except imggen.ApiError as exc:
            entry = {"n": i + 1, "error": str(exc)[:400]}
            tokens = str(exc).replace('"', ' ').replace(':', ' ').split()
            dropped = [p for p in FALLBACK_PARAMS if p in params and p in tokens]
            if dropped:
                for p in dropped:
                    params.pop(p)
                entry["dropped_params"] = dropped
                record["attempts"].append(entry)
                continue
            record["attempts"].append(entry)
        except (RuntimeError, OSError) as exc:
            record["attempts"].append({"n": i + 1, "error": str(exc)[:400]})
        else:
            record["ok"] = True
            record["params"] = params
            record["attempts"].append({
                "n": i + 1, "ok": True,
                "elapsed_s": round(time.time() - t0, 1), **outcome})
            break
        time.sleep(10 * (i + 1))
    print("[run] %s ok=%s attempts=%d file=%s" %
          (spec["id"], record["ok"], len(record["attempts"]), spec["file"]))
    return record


def merge_results(new_records):
    existing = []
    if RESULTS.exists():
        existing = json.loads(RESULTS.read_text()).get("records", [])
    by_id = {r["id"]: r for r in existing}
    for r in new_records:
        old = by_id.get(r["id"])
        if old:
            prev = old.get("prev_runs", [])
            prev.append({"ok": old["ok"], "attempts": old["attempts"]})
            r["prev_runs"] = prev
        by_id[r["id"]] = r
    ordered = [by_id[s["id"]] for s in SPECS if s["id"] in by_id]
    RESULTS.write_text(json.dumps({"records": ordered}, indent=2,
                                  ensure_ascii=False))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--only", action="append", help="spec id, repeatable")
    ap.add_argument("--max-attempts", type=int, default=2, help="race waves")
    ap.add_argument("--race", type=int, default=3, help="concurrent requests per wave")
    ap.add_argument("--drop", default="", help="comma-separated params to strip")
    args = ap.parse_args()
    env = imggen.load_env()
    selected = [s for s in SPECS if not args.only or s["id"] in args.only]
    if not selected:
        raise SystemExit("no spec matched --only %s" % args.only)
    for p in [x for x in args.drop.split(",") if x]:
        for spec in selected:
            spec["params"].pop(p, None)
    records = []
    for spec in selected:
        records.append(run_spec(spec, env, args.max_attempts, args.race))
    merge_results(records)
    failed = [r["id"] for r in records if not r["ok"]]
    if failed:
        raise SystemExit("FAILED: %s" % ",".join(failed))
    print("[run] all selected specs generated")


if __name__ == "__main__":
    main()
