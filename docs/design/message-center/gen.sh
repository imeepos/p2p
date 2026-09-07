#!/usr/bin/env bash
# gpt-image-2 generation via the FIRST OPENAI_API_KEY/OPENAI_BASE_URL pair in .env
# usage: gen.sh <payload.json> <out.png>
set -euo pipefail
cd "$(git rev-parse --show-toplevel 2>/dev/null || echo .)"
K1=$(grep "^OPENAI_API_KEY=" .env | head -1 | cut -d= -f2-)
B1=$(grep "^OPENAI_BASE_URL=" .env | head -1 | cut -d= -f2-)
payload="$1"; out="$2"
resp="$out.response.json"
code=$(curl -sS -m 270 -o "$resp" -w "%{http_code}" \
  -H "Authorization: Bearer $K1" -H "Content-Type: application/json" \
  -d @"$payload" "$B1/v1/images/generations")
echo "HTTP:$code"
if [ "$code" != "200" ]; then head -c 300 "$resp"; echo; exit 1; fi
python3 - "$resp" "$out" <<'PY'
import base64, json, os, sys
data = json.load(open(sys.argv[1]))
img = base64.b64decode(data["data"][0]["b64_json"])
open(sys.argv[2], "wb").write(img)
print("SAVED", sys.argv[2], os.path.getsize(sys.argv[2]), "bytes")
PY
rm -f "$resp"
