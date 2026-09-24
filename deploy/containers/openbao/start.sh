#!/bin/sh
# SPDX-License-Identifier: MPL-2.0
set -eu

export BAO_ADDR=http://127.0.0.1:8200
bao server -config=/etc/openbao/config.hcl &
server_pid=$!

stop_server() {
    kill -TERM "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
}
trap stop_server HUP INT TERM EXIT

for attempt in $(seq 1 120); do
    status="$(bao status -address="$BAO_ADDR" -format=json 2>/dev/null || true)"
    if printf '%s' "$status" | grep -q '"initialized"'; then
        break
    fi
    sleep 1
done
if ! printf '%s' "$status" | grep -q '"initialized"'; then
    echo 'OpenBao did not become reachable' >&2
    exit 1
fi

/bin/sh /usr/local/bin/arqen-openbao-bootstrap.sh
wait "$server_pid"
