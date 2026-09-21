#!/bin/sh
set -eu

bao_addr="${BAO_ADDR:-http://openbao:8200}"
key_file="/run/secrets/arqen-openbao-unseal-key"

status_json=''
for attempt in $(seq 1 120); do
    status_json="$(bao status -address="$bao_addr" -format=json 2>/dev/null || true)"
    if printf '%s' "$status_json" | grep -q '"initialized"'; then
        break
    fi
    sleep 1
done

printf '%s' "$status_json" | grep -Eq '"initialized"[[:space:]]*:[[:space:]]*true' || {
    echo 'OpenBao did not become reachable' >&2
    exit 1
}

printf '%s' "$status_json" | grep -Eq '"sealed"[[:space:]]*:[[:space:]]*true' || exit 0
test -s "$key_file" || {
    echo "missing OpenBao unseal secret at $key_file" >&2
    exit 1
}
bao operator unseal -address="$bao_addr" "$(cat "$key_file")" >/dev/null
printf '%s\n' 'OpenBao unsealed.'
