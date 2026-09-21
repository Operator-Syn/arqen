#!/bin/sh
set -eu

bao_addr="${BAO_ADDR:-http://openbao:8200}"
setup_dir="${ARQEN_OPENBAO_SETUP_DIR:-/run/arqen/setup}"
unseal_file="$setup_dir/openbao-unseal-key"
root_token_file="$setup_dir/openbao-bootstrap-token"
control_role_id_file="$setup_dir/openbao-control-role-id"
control_secret_id_file="$setup_dir/openbao-control-secret-id"
broker_role_id_file="$setup_dir/openbao-broker-role-id"
broker_secret_id_file="$setup_dir/openbao-broker-secret-id"

mkdir -p "$setup_dir"
chmod 700 "$setup_dir"

json_string() {
    field="$1"
    printf '%s' "$2" \
        | sed -n "s/.*\"$field\"[[:space:]]*:[[:space:]]*\"\([^\"]*\)\".*/\1/p" \
        | head -n 1
}

json_unseal_key() {
    printf '%s' "$1" \
        | sed -n '/"unseal_keys_b64"/{n; s/.*"\([^"]*\)".*/\1/p;}' \
        | head -n 1
}

status_json=''
for attempt in $(seq 1 120); do
    status_json="$(bao status -address="$bao_addr" -format=json 2>/dev/null || true)"
    if printf '%s' "$status_json" | grep -q '"initialized"'; then
        break
    fi
    sleep 1
done

if ! printf '%s' "$status_json" | grep -q '"initialized"'; then
    echo 'OpenBao did not become reachable during setup' >&2
    exit 1
fi

if printf '%s' "$status_json" | grep -Eq '"initialized"[[:space:]]*:[[:space:]]*false'; then
    init_json="$(bao operator init \
        -address="$bao_addr" \
        -format=json \
        -key-shares=1 \
        -key-threshold=1)"
    json_unseal_key "$init_json" > "$unseal_file"
    json_string root_token "$init_json" > "$root_token_file"
    chmod 600 "$unseal_file" "$root_token_file"
fi

test -s "$unseal_file" || {
    echo "missing OpenBao unseal key at $unseal_file" >&2
    exit 1
}
test -s "$root_token_file" || {
    echo "missing OpenBao bootstrap token at $root_token_file" >&2
    exit 1
}

status_json="$(bao status -address="$bao_addr" -format=json 2>/dev/null || true)"
if printf '%s' "$status_json" | grep -Eq '"sealed"[[:space:]]*:[[:space:]]*true'; then
    bao operator unseal -address="$bao_addr" "$(cat "$unseal_file")" >/dev/null
fi

export BAO_ADDR="$bao_addr"
export BAO_TOKEN="$(cat "$root_token_file")"

if ! bao secrets list -format=json 2>/dev/null | grep -q '"secret/"'; then
    bao secrets enable -path=secret kv-v2 >/dev/null
fi
if ! bao auth list -format=json 2>/dev/null | grep -q '"approle/"'; then
    bao auth enable approle >/dev/null
fi

bao policy write arqen-control /etc/arqen/policy-control.hcl >/dev/null
bao policy write arqen-broker /etc/arqen/policy-broker.hcl >/dev/null

bao write auth/approle/role/arqen-control \
    token_policies=arqen-control \
    token_type=service \
    token_ttl=1h \
    token_max_ttl=4h \
    secret_id_ttl=0 \
    secret_id_num_uses=0 >/dev/null
bao write auth/approle/role/arqen-broker \
    token_policies=arqen-broker \
    token_type=service \
    token_ttl=1h \
    token_max_ttl=4h \
    secret_id_ttl=0 \
    secret_id_num_uses=0 >/dev/null

if [ ! -s "$control_role_id_file" ]; then
    bao read -field=role_id auth/approle/role/arqen-control/role-id > "$control_role_id_file"
fi
if [ ! -s "$control_secret_id_file" ]; then
    bao write -f -field=secret_id auth/approle/role/arqen-control/secret-id > "$control_secret_id_file"
fi
if [ ! -s "$broker_role_id_file" ]; then
    bao read -field=role_id auth/approle/role/arqen-broker/role-id > "$broker_role_id_file"
fi
if [ ! -s "$broker_secret_id_file" ]; then
    bao write -f -field=secret_id auth/approle/role/arqen-broker/secret-id > "$broker_secret_id_file"
fi

chmod 600 \
    "$unseal_file" \
    "$root_token_file" \
    "$control_role_id_file" \
    "$control_secret_id_file" \
    "$broker_role_id_file" \
    "$broker_secret_id_file"

secret_uid="${ARQEN_OPENBAO_SECRET_UID:-0}"
secret_gid="${ARQEN_OPENBAO_SECRET_GID:-0}"
case "$secret_uid" in
    ''|*[!0-9]*)
        echo 'ARQEN_OPENBAO_SECRET_UID must be numeric' >&2
        exit 1
        ;;
esac
case "$secret_gid" in
    ''|*[!0-9]*)
        echo 'ARQEN_OPENBAO_SECRET_GID must be numeric' >&2
        exit 1
        ;;
esac
chown "$secret_uid:$secret_gid" \
    "$unseal_file" \
    "$root_token_file" \
    "$control_role_id_file" \
    "$control_secret_id_file" \
    "$broker_role_id_file" \
    "$broker_secret_id_file"

printf '%s\n' 'OpenBao initialized, unsealed, and configured for Arqen.'
