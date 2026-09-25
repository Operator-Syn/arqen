#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
set -euo pipefail

image="${1:?usage: arqen-openbao-image-smoke.sh IMAGE_REFERENCE}"
suffix="$$-${RANDOM}"
container_name="arqen-openbao-image-smoke-$suffix"
volume_prefix="arqen-openbao-image-smoke-$suffix"
volumes=(
    "$volume_prefix-data"
    "$volume_prefix-admin"
    "$volume_prefix-control"
    "$volume_prefix-broker"
    "$volume_prefix-control-secret"
    "$volume_prefix-mcp-secret"
)

cleanup() {
    docker container rm --force "$container_name" >/dev/null 2>&1 || true
    docker volume rm "${volumes[@]}" >/dev/null 2>&1 || true
}
trap cleanup EXIT

docker volume create "${volumes[0]}" >/dev/null
docker volume create "${volumes[1]}" >/dev/null
docker volume create "${volumes[2]}" >/dev/null
docker volume create "${volumes[3]}" >/dev/null
docker volume create "${volumes[4]}" >/dev/null
docker volume create "${volumes[5]}" >/dev/null

# Model the empty data volume created by the previously published image.
docker run --rm \
    --volume "${volumes[0]}:/openbao/file" \
    --entrypoint /bin/sh \
    "$image" \
    -c 'chown 100:1000 /openbao/file && chmod 755 /openbao/file'

docker run --detach --name "$container_name" \
    --cap-drop ALL \
    --cap-add CHOWN \
    --user 0:0 \
    --env ARQEN_OPENBAO_SETUP_DIR=/run/arqen/admin \
    --env ARQEN_OPENBAO_CONTROL_DIR=/run/arqen/control \
    --env ARQEN_OPENBAO_BROKER_DIR=/run/arqen/broker \
    --env ARQEN_CONTROL_SECRET_DIR=/run/arqen/control-secret \
    --env ARQEN_MCP_SECRET_DIR=/run/arqen/mcp-secret \
    --env ARQEN_OPENBAO_SECRET_UID=65532 \
    --env ARQEN_OPENBAO_SECRET_GID=0 \
    --volume "${volumes[0]}:/openbao/file" \
    --volume "${volumes[1]}:/run/arqen/admin" \
    --volume "${volumes[2]}:/run/arqen/control" \
    --volume "${volumes[3]}:/run/arqen/broker" \
    --volume "${volumes[4]}:/run/arqen/control-secret" \
    --volume "${volumes[5]}:/run/arqen/mcp-secret" \
    "$image" >/dev/null

wait_until_ready() {
    local status
    for _attempt in $(seq 1 60); do
        if docker exec "$container_name" sh -c '
            test -s /run/arqen/admin/openbao-bootstrap-ready &&
            test -x /etc/openbao &&
            test -r /etc/openbao/config.hcl &&
            test -x /etc/arqen &&
            test -r /etc/arqen/policy-control.hcl &&
            test -r /etc/arqen/policy-broker.hcl &&
            test -w /openbao/file &&
            test -s /run/arqen/control/openbao-control-role-id &&
            test -s /run/arqen/control/openbao-control-secret-id &&
            test -s /run/arqen/broker/openbao-broker-role-id &&
            test -s /run/arqen/broker/openbao-broker-secret-id &&
            test -s /run/arqen/control-secret/control-password &&
            test -s /run/arqen/mcp-secret/mcp-bearer-token &&
            bao status -address=http://127.0.0.1:8200 -format=json 2>/dev/null \
                | grep -q "sealed.*false"'; then
            return 0
        fi

        status="$(docker inspect --format '{{.State.Status}}' "$container_name")"
        if [[ "$status" == exited || "$status" == dead ]]; then
            docker logs "$container_name" 2>&1 | tail -n 20 >&2
            return 1
        fi
        sleep 0.5
    done

    docker logs "$container_name" 2>&1 | tail -n 20 >&2
    return 1
}

credential_checksums() {
    docker exec --user 65532:0 "$container_name" cksum \
        /run/arqen/control/openbao-control-role-id \
        /run/arqen/control/openbao-control-secret-id \
        /run/arqen/broker/openbao-broker-role-id \
        /run/arqen/broker/openbao-broker-secret-id \
        /run/arqen/control-secret/control-password \
        /run/arqen/mcp-secret/mcp-bearer-token
}

wait_until_ready
credentials_before_restart="$(credential_checksums)"
docker restart --time 2 "$container_name" >/dev/null
wait_until_ready
credentials_after_restart="$(credential_checksums)"

if [[ "$credentials_before_restart" != "$credentials_after_restart" ]]; then
    echo 'OpenBao regenerated credentials on restart.' >&2
    exit 1
fi

printf 'OpenBao image smoke passed for %s.\n' "$image"
