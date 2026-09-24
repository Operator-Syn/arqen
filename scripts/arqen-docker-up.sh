#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
set -euo pipefail

script_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck source=lib/common.sh
. "$script_directory/lib/common.sh"
arqen_load_defaults
arqen_prepare_runtime
arqen_require_docker_compose
arqen_require_command curl
image_source="${ARQEN_DOCKER_IMAGE_SOURCE:-build}"
case "$image_source" in
    build|registry|bundle) ;;
    *) arqen_die 'ARQEN_DOCKER_IMAGE_SOURCE must be build, registry, or bundle' ;;
esac
bundle_file="${ARQEN_DOCKER_IMAGE_BUNDLE:-out/arqen-docker-stack/docker-images-linux-amd64.tar}"
if [[ "$image_source" == bundle && ! -f "$bundle_file" ]]; then
    arqen_die "Docker image bundle not found: $bundle_file (run make docker-bundle)"
fi
arqen_detect_docker_display

secrets_directory="$ARQEN_ROOT/.secrets"
if [[ ! -s "$secrets_directory/openbao-unseal-key" || ! -s "$secrets_directory/openbao-control-role-id" ]]; then
    "$script_directory/arqen-docker-setup.sh"
    set -a
    # shellcheck disable=SC1091
    . "$ARQEN_ROOT/.env"
    set +a
    arqen_load_defaults
    arqen_prepare_runtime
    arqen_detect_docker_display
fi

arqen_require_google_client
arqen_require_token

compose_file="$ARQEN_ROOT/deploy/containers/docker-native-compose.yml"
compose_project="${ARQEN_DOCKER_PROJECT:-arqen-docker}"
compose_args=(
    --project-name "$compose_project"
    --file "$compose_file"
    --file "$ARQEN_DOCKER_DISPLAY_COMPOSE_FILE"
)
ARQEN_DOCKER_SECRET_UID="$(id -u)"
ARQEN_DOCKER_SECRET_GID="$(id -g)"
export ARQEN_ROOT GOOGLE_CLIENT_SECRET ARQEN_MCP_BEARER_TOKEN_FILE
export ARQEN_DOCKER_SECRETS_DIR="$secrets_directory"
export ARQEN_DOCKER_SECRET_UID ARQEN_DOCKER_SECRET_GID
export ARQEN_MCP_ALLOWED_HOSTS ARQEN_MCP_ALLOWED_ORIGINS
export ARQEN_DOCKER_HOST_UID ARQEN_DOCKER_HOST_GID
export ARQEN_DOCKER_IMAGE_TAG="${ARQEN_DOCKER_IMAGE_TAG:-latest}"

docker compose "${compose_args[@]}" \
    up -d openbao
docker compose "${compose_args[@]}" \
    --profile ops \
    run --rm openbao-unseal
docker compose "${compose_args[@]}" \
    --profile ops \
    run --rm --no-deps openbao-bootstrap
docker compose "${compose_args[@]}" \
    --profile ops \
    run --rm --no-deps arqen-secret-init
case "$image_source" in
    build)
        docker compose "${compose_args[@]}" \
            up --build -d arqen-broker arqen-control arqen-mcp
        ;;
    registry)
        docker compose "${compose_args[@]}" \
            pull arqen-broker arqen-control arqen-mcp
        docker compose "${compose_args[@]}" \
            up --no-build -d arqen-broker arqen-control arqen-mcp
        ;;
    bundle)
        docker image load --input "$bundle_file"
        docker compose "${compose_args[@]}" \
            up --no-build -d arqen-broker arqen-control arqen-mcp
        ;;
esac

control_url="http://127.0.0.1:${ARQEN_CONTROL_HOST_PORT:-7681}"
control_status=''
for _attempt in $(seq 1 120); do
    control_status="$(curl -sS --max-time 2 -o /dev/null -w '%{http_code}' \
        "$control_url/" 2>/dev/null || true)"
    if [[ "$control_status" == 200 ]]; then
        break
    fi
    sleep 0.5
done
[[ "$control_status" == 200 ]] || arqen_die "Arqen control gateway did not become reachable at $control_url"

openbao_status=''
for _attempt in $(seq 1 120); do
    openbao_status="$(docker compose "${compose_args[@]}" \
        exec -T -e BAO_ADDR=http://openbao:8200 openbao bao status -format=json 2>/dev/null || true)"
    if printf '%s' "$openbao_status" \
        | grep -Eq '"initialized"[[:space:]]*:[[:space:]]*true' \
        && printf '%s' "$openbao_status" \
        | grep -Eq '"sealed"[[:space:]]*:[[:space:]]*false'; then
        break
    fi
    sleep 0.5
done
if ! printf '%s' "$openbao_status" \
    | grep -Eq '"initialized"[[:space:]]*:[[:space:]]*true' \
    || ! printf '%s' "$openbao_status" \
        | grep -Eq '"sealed"[[:space:]]*:[[:space:]]*false'; then
    arqen_die 'OpenBao did not become initialized and unsealed'
fi

broker_ready=false
for _attempt in $(seq 1 120); do
    if docker compose "${compose_args[@]}" \
        exec -T arqen-broker test -S /run/arqen/gmail-broker.sock \
        >/dev/null 2>&1; then
        broker_ready=true
        break
    fi
    sleep 0.5
done
[[ "$broker_ready" == true ]] || arqen_die 'credential broker did not create its Unix socket'

token="$(arqen_read_token)"
base_url="http://127.0.0.1:${ARQEN_MCP_HOST_PORT:-8787}"
health_status=''
for _attempt in $(seq 1 120); do
    health_status="$(curl -sS --max-time 2 -o /dev/null -w '%{http_code}' \
        -H "Authorization: Bearer $token" "$base_url/healthz" 2>/dev/null || true)"
    if [[ "$health_status" == 204 ]]; then
        break
    fi
    sleep 0.5
done
[[ "$health_status" == 204 ]] || arqen_die "Docker-native MCP did not become live at $base_url"

ready_status=''
for _attempt in $(seq 1 120); do
    ready_status="$(curl -sS --max-time 2 \
        -H "Authorization: Bearer $token" \
        -o /dev/null -w '%{http_code}' "$base_url/readyz" 2>/dev/null || true)"
    if [[ "$ready_status" == 204 ]]; then
        break
    fi
    if [[ "$ready_status" == 503 ]]; then
        break
    fi
    sleep 0.5
done
[[ "$ready_status" == 204 || "$ready_status" == 503 ]] \
    || arqen_die "Docker-native MCP readiness endpoint did not respond at $base_url"

printf 'Docker-native Arqen is running.\n'
printf '  TUI:  http://127.0.0.1:%s\n' "${ARQEN_CONTROL_HOST_PORT:-7681}"
printf '  OAuth callback: http://127.0.0.1:%s/oauth2/callback\n' "${ARQEN_OAUTH_CALLBACK_HOST_PORT:-8765}"
printf '  MCP:  %s/mcp\n' "$base_url"
printf '  Readiness depends on the account selected in the streamed TUI.\n'
