#!/bin/sh
# SPDX-License-Identifier: MPL-2.0
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
umask 077

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

login_error_file="$(mktemp "$setup_dir/.approle-login-error.XXXXXX")"
login_role_id_file="$(mktemp "$setup_dir/.approle-login-role-id.XXXXXX")"
login_secret_id_file="$(mktemp "$setup_dir/.approle-login-secret-id.XXXXXX")"
role_id_temp=''
secret_id_temp=''
login_http_status=''
cleanup() {
    rm -f "$login_error_file" "$login_role_id_file" "$login_secret_id_file"
    [ -z "$role_id_temp" ] || rm -f "$role_id_temp"
    [ -z "$secret_id_temp" ] || rm -f "$secret_id_temp"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

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
for _attempt in $(seq 1 120); do
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
BAO_TOKEN="$(cat "$root_token_file")"
export BAO_TOKEN

if ! bao secrets list -format=json 2>/dev/null | grep -q '"secret/"'; then
    bao secrets enable -path=secret kv-v2 >/dev/null
fi
if ! bao auth list -format=json 2>/dev/null | grep -q '"approle/"'; then
    bao auth enable approle >/dev/null
fi

configure_role() {
    config_role="$1"
    config_policy_file="$2"
    bao policy write "$config_role" "$config_policy_file" >/dev/null
    bao write "auth/approle/role/$config_role" \
        "token_policies=$config_role" \
        token_type=service \
        token_ttl=1h \
        token_max_ttl=4h \
        secret_id_ttl=0 \
        secret_id_num_uses=0 >/dev/null
}

verify_role_login() {
    verify_role_id_path="$1"
    verify_secret_id_path="$2"
    tr -d '\r\n' < "$verify_role_id_path" > "$login_role_id_file"
    tr -d '\r\n' < "$verify_secret_id_path" > "$login_secret_id_file"
    : > "$login_error_file"
    if bao write -format=json auth/approle/login \
        "role_id=@$login_role_id_file" \
        "secret_id=@$login_secret_id_file" \
        >/dev/null 2>"$login_error_file"; then
        return 0
    fi
    login_http_status="$(sed -n 's/.*Code: \([0-9][0-9][0-9]\).*/\1/p' "$login_error_file" | head -n 1)"
    if [ "$login_http_status" = 400 ]; then
        return 1
    fi
    return 2
}

reconcile_role() {
    reconcile_role_name="$1"
    reconcile_policy_file="$2"
    reconcile_role_id_path="$3"
    reconcile_secret_id_path="$4"

    configure_role "$reconcile_role_name" "$reconcile_policy_file"

    role_id_temp="$(mktemp "$setup_dir/.$reconcile_role_name-role-id.XXXXXX")"
    secret_id_temp="$(mktemp "$setup_dir/.$reconcile_role_name-secret-id.XXXXXX")"
    if ! bao delete "auth/approle/role/$reconcile_role_name" >/dev/null 2>&1; then
        echo "Could not reset the Arqen $reconcile_role_name AppRole; existing credentials were left in place." >&2
        return 1
    fi
    configure_role "$reconcile_role_name" "$reconcile_policy_file"
    if ! bao read -field=role_id "auth/approle/role/$reconcile_role_name/role-id" > "$role_id_temp" 2>/dev/null; then
        echo "Could not create the Arqen $reconcile_role_name AppRole identifier." >&2
        return 1
    fi
    if ! bao write -f -field=secret_id "auth/approle/role/$reconcile_role_name/secret-id" > "$secret_id_temp" 2>/dev/null; then
        echo "Could not create the Arqen $reconcile_role_name AppRole secret." >&2
        return 1
    fi
    chmod 600 "$role_id_temp" "$secret_id_temp"
    chown "$secret_uid:$secret_gid" "$role_id_temp" "$secret_id_temp"
    if verify_role_login "$role_id_temp" "$secret_id_temp"; then
        :
    else
        verification_status=$?
        if [ "$verification_status" -eq 1 ]; then
            echo "OpenBao rejected the newly created Arqen $reconcile_role_name AppRole credentials; startup was stopped." >&2
        else
            if [ -n "$login_http_status" ]; then
                echo "OpenBao returned HTTP $login_http_status while verifying the new Arqen $reconcile_role_name AppRole credentials; startup was stopped." >&2
            else
                echo "OpenBao did not complete verification of the new Arqen $reconcile_role_name AppRole credentials; startup was stopped." >&2
            fi
        fi
        return 1
    fi
    mv -f "$role_id_temp" "$reconcile_role_id_path"
    mv -f "$secret_id_temp" "$reconcile_secret_id_path"
    role_id_temp=''
    secret_id_temp=''
    printf 'Repaired Arqen %s AppRole credentials.\n' "$reconcile_role_name"
}

ensure_role() {
    ensure_role_name="$1"
    ensure_policy_file="$2"
    ensure_role_id_path="$3"
    ensure_secret_id_path="$4"

    if [ ! -s "$ensure_role_id_path" ] || [ ! -s "$ensure_secret_id_path" ]; then
        reconcile_role "$ensure_role_name" "$ensure_policy_file" "$ensure_role_id_path" "$ensure_secret_id_path"
        return
    fi

    if verify_role_login "$ensure_role_id_path" "$ensure_secret_id_path"; then
        return
    else
        ensure_login_status=$?
    fi
    case "$ensure_login_status" in
        1)
            reconcile_role "$ensure_role_name" "$ensure_policy_file" "$ensure_role_id_path" "$ensure_secret_id_path"
            ;;
        *)
            echo "Could not verify the Arqen $ensure_role_name AppRole because OpenBao did not complete the login request; no credentials were rotated." >&2
            return 1
            ;;
    esac
}

ensure_role arqen-control /etc/arqen/policy-control.hcl "$control_role_id_file" "$control_secret_id_file"
ensure_role arqen-broker /etc/arqen/policy-broker.hcl "$broker_role_id_file" "$broker_secret_id_file"

chmod 600 \
    "$unseal_file" \
    "$root_token_file" \
    "$control_role_id_file" \
    "$control_secret_id_file" \
    "$broker_role_id_file" \
    "$broker_secret_id_file"

chown "$secret_uid:$secret_gid" \
    "$unseal_file" \
    "$root_token_file" \
    "$control_role_id_file" \
    "$control_secret_id_file" \
    "$broker_role_id_file" \
    "$broker_secret_id_file"

printf '%s\n' 'OpenBao initialized, unsealed, and configured for Arqen.'
