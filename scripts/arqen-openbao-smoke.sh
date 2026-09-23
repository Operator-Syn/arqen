#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
image="openbao/openbao:2.6.0"
container_name="arqen-openbao-smoke-$$"
setup_dir="$(mktemp -d)"
test_root_token='arqen-smoke-root-token-only'
started=false

cleanup() {
    if [[ "$started" == true ]]; then
        docker rm --force "$container_name" >/dev/null 2>&1 || true
    fi
    rm -rf "$setup_dir"
}
trap cleanup EXIT

mkdir -m 700 "$setup_dir/secrets"
printf '%s\n' "$test_root_token" > "$setup_dir/secrets/openbao-bootstrap-token"
printf '%s\n' 'test-only-unseal-placeholder' > "$setup_dir/secrets/openbao-unseal-key"
chmod 600 "$setup_dir/secrets/openbao-bootstrap-token" "$setup_dir/secrets/openbao-unseal-key"

docker run --detach --name "$container_name" \
    "$image" server -dev -dev-listen-address=0.0.0.0:8200 \
    "-dev-root-token-id=$test_root_token" >/dev/null
started=true

ready=false
for _attempt in $(seq 1 60); do
    if docker exec --env BAO_ADDR=http://127.0.0.1:8200 \
        "$container_name" bao status >/dev/null 2>&1; then
        ready=true
        break
    fi
    sleep 0.25
done
[[ "$ready" == true ]] || {
    printf '%s\n' 'Disposable OpenBao did not become ready.' >&2
    exit 1
}

run_bootstrap() {
    docker run --rm --network "container:$container_name" --user 0:0 \
        --env BAO_ADDR=http://127.0.0.1:8200 \
        --env ARQEN_OPENBAO_SETUP_DIR=/run/arqen/setup \
        --env "ARQEN_OPENBAO_SECRET_UID=$(id -u)" \
        --env "ARQEN_OPENBAO_SECRET_GID=$(id -g)" \
        --volume "$setup_dir/secrets:/run/arqen/setup" \
        --volume "$repository_root/deploy/containers/openbao/bootstrap.sh:/run/arqen/bootstrap.sh:ro" \
        --volume "$repository_root/deploy/containers/openbao/policy-control.hcl:/etc/arqen/policy-control.hcl:ro" \
        --volume "$repository_root/deploy/containers/openbao/policy-broker.hcl:/etc/arqen/policy-broker.hcl:ro" \
        --entrypoint /bin/sh "$image" /run/arqen/bootstrap.sh
}

assert_not_printed() {
    local output="$1"
    local file="$2"
    local value
    value="$(cat "$file")"
    if [[ "$output" == *"$value"* ]]; then
        printf '%s\n' 'Bootstrap output contained a protected test value.' >&2
        exit 1
    fi
}

assert_role_configuration() {
    local role="$1"
    local role_settings
    local policy
    role_settings="$(docker exec --env BAO_ADDR=http://127.0.0.1:8200 --env "BAO_TOKEN=$test_root_token" \
        "$container_name" bao read -format=json "auth/approle/role/$role")"
    grep -Eq '"token_ttl"[[:space:]]*:[[:space:]]*3600' <<< "$role_settings" || {
        printf 'OpenBao smoke: %s token TTL was not restored.\n' "$role" >&2
        exit 1
    }
    grep -Eq '"token_max_ttl"[[:space:]]*:[[:space:]]*14400' <<< "$role_settings" || {
        printf 'OpenBao smoke: %s maximum token TTL was not restored.\n' "$role" >&2
        exit 1
    }
    grep -Eq '"secret_id_ttl"[[:space:]]*:[[:space:]]*0' <<< "$role_settings" || {
        printf 'OpenBao smoke: %s SecretID TTL was not restored.\n' "$role" >&2
        exit 1
    }
    grep -Eq '"secret_id_num_uses"[[:space:]]*:[[:space:]]*0' <<< "$role_settings" || {
        printf 'OpenBao smoke: %s SecretID use limit was not restored.\n' "$role" >&2
        exit 1
    }
    policy="$(docker exec --env BAO_ADDR=http://127.0.0.1:8200 --env "BAO_TOKEN=$test_root_token" \
        "$container_name" bao policy read "$role")"
    [[ "$policy" == *'secret/data/arqen/google/'* ]] || {
        printf 'OpenBao smoke: %s policy was not restored.\n' "$role" >&2
        exit 1
    }
}

output="$(run_bootstrap)"
[[ "$output" == *"Repaired Arqen arqen-control AppRole credentials."* ]]
[[ "$output" == *"Repaired Arqen arqen-broker AppRole credentials."* ]]
[[ "$output" != *"$test_root_token"* ]]
assert_not_printed "$output" "$setup_dir/secrets/openbao-control-role-id"
assert_not_printed "$output" "$setup_dir/secrets/openbao-control-secret-id"
assert_not_printed "$output" "$setup_dir/secrets/openbao-broker-role-id"
assert_not_printed "$output" "$setup_dir/secrets/openbao-broker-secret-id"
assert_role_configuration arqen-control
assert_role_configuration arqen-broker

docker exec --env BAO_ADDR=http://127.0.0.1:8200 --env "BAO_TOKEN=$test_root_token" \
    "$container_name" bao kv put secret/arqen/google/smoke marker=preserve-this-value >/dev/null

control_before="$(cksum "$setup_dir/secrets/openbao-control-role-id" "$setup_dir/secrets/openbao-control-secret-id")"
broker_before="$(cksum "$setup_dir/secrets/openbao-broker-role-id" "$setup_dir/secrets/openbao-broker-secret-id")"
output="$(run_bootstrap)"
[[ "$output" != *"Repaired Arqen"* ]]
[[ "$control_before" == "$(cksum "$setup_dir/secrets/openbao-control-role-id" "$setup_dir/secrets/openbao-control-secret-id")" ]]
[[ "$broker_before" == "$(cksum "$setup_dir/secrets/openbao-broker-role-id" "$setup_dir/secrets/openbao-broker-secret-id")" ]]

printf '%s\n' 'stale-control-role-id' > "$setup_dir/secrets/openbao-control-role-id"
chmod 600 "$setup_dir/secrets/openbao-control-role-id"
broker_before="$(cksum "$setup_dir/secrets/openbao-broker-role-id" "$setup_dir/secrets/openbao-broker-secret-id")"
output="$(run_bootstrap)"
[[ "$output" == *"Repaired Arqen arqen-control AppRole credentials."* ]]
[[ "$output" != *"Repaired Arqen arqen-broker AppRole credentials."* ]]
[[ "$broker_before" == "$(cksum "$setup_dir/secrets/openbao-broker-role-id" "$setup_dir/secrets/openbao-broker-secret-id")" ]]
assert_role_configuration arqen-control

printf '%s\n' 'stale-broker-secret-id' > "$setup_dir/secrets/openbao-broker-secret-id"
chmod 600 "$setup_dir/secrets/openbao-broker-secret-id"
control_before="$(cksum "$setup_dir/secrets/openbao-control-role-id" "$setup_dir/secrets/openbao-control-secret-id")"
output="$(run_bootstrap)"
[[ "$output" == *"Repaired Arqen arqen-broker AppRole credentials."* ]]
[[ "$output" != *"Repaired Arqen arqen-control AppRole credentials."* ]]
[[ "$control_before" == "$(cksum "$setup_dir/secrets/openbao-control-role-id" "$setup_dir/secrets/openbao-control-secret-id")" ]]

preserved_marker="$(docker exec --env BAO_ADDR=http://127.0.0.1:8200 --env "BAO_TOKEN=$test_root_token" \
    "$container_name" bao kv get -field=marker secret/arqen/google/smoke)"
[[ "$preserved_marker" == 'preserve-this-value' ]]

mkdir -m 700 "$setup_dir/unavailable-bin"
cat > "$setup_dir/unavailable-bin/bao" <<'MOCK_BAO'
#!/bin/sh
case "$1" in
    status)
        printf '%s\n' '{"initialized":true,"sealed":false}'
        ;;
    secrets)
        printf '%s\n' '{"secret/":{}}'
        ;;
    auth)
        printf '%s\n' '{"approle/":{}}'
        ;;
    write)
        printf '%s\n' 'simulated OpenBao transport failure' >&2
        exit 1
        ;;
    *)
        exit 1
        ;;
esac
MOCK_BAO
chmod 700 "$setup_dir/unavailable-bin/bao"
control_before="$(cksum "$setup_dir/secrets/openbao-control-role-id" "$setup_dir/secrets/openbao-control-secret-id")"
broker_before="$(cksum "$setup_dir/secrets/openbao-broker-role-id" "$setup_dir/secrets/openbao-broker-secret-id")"
if unavailable_output="$(docker run --rm --network "container:$container_name" --user 0:0 \
    --env BAO_ADDR=http://openbao-unavailable:8200 \
    --env ARQEN_OPENBAO_SETUP_DIR=/run/arqen/setup \
    --env ARQEN_OPENBAO_SECRET_UID="$(id -u)" \
    --env ARQEN_OPENBAO_SECRET_GID="$(id -g)" \
    --env PATH=/run/arqen/test-bin:/usr/bin:/bin \
    --volume "$setup_dir/secrets:/run/arqen/setup" \
    --volume "$setup_dir/unavailable-bin:/run/arqen/test-bin:ro" \
    --volume "$repository_root/deploy/containers/openbao/bootstrap.sh:/run/arqen/bootstrap.sh:ro" \
    --volume "$repository_root/deploy/containers/openbao/policy-control.hcl:/etc/arqen/policy-control.hcl:ro" \
    --volume "$repository_root/deploy/containers/openbao/policy-broker.hcl:/etc/arqen/policy-broker.hcl:ro" \
    --entrypoint /bin/sh "$image" /run/arqen/bootstrap.sh 2>&1)"; then
    printf '%s\n' 'Bootstrap unexpectedly succeeded while AppRole login was unavailable.' >&2
    exit 1
fi
[[ "$unavailable_output" == *"no credentials were rotated"* ]]
[[ "$unavailable_output" != *"$test_root_token"* ]]
[[ "$unavailable_output" != *"simulated OpenBao transport failure"* ]]
[[ "$control_before" == "$(cksum "$setup_dir/secrets/openbao-control-role-id" "$setup_dir/secrets/openbao-control-secret-id")" ]]
[[ "$broker_before" == "$(cksum "$setup_dir/secrets/openbao-broker-role-id" "$setup_dir/secrets/openbao-broker-secret-id")" ]]

printf '%s\n' 'OpenBao smoke passed: disposable service repair, isolation, idempotence, KV preservation, and no-rotation on unavailable auth.'
