#!/usr/bin/env bash
set -euo pipefail

script_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/common.sh
. "$script_directory/lib/common.sh"
arqen_load_defaults
arqen_prepare_runtime
arqen_require_docker_compose
arqen_require_command curl
arqen_require_token

compose_file="$ARQEN_ROOT/deploy/containers/docker-native-compose.yml"
compose_project="${ARQEN_DOCKER_PROJECT:-arqen-docker}"
export ARQEN_ROOT
export ARQEN_DOCKER_SECRETS_DIR="$ARQEN_ROOT/.secrets"

docker compose \
    --project-name "$compose_project" \
    --file "$compose_file" \
    ps

control_url="http://127.0.0.1:${ARQEN_CONTROL_HOST_PORT:-7681}"
printf '\nControl gateway: '
curl -sS --max-time 2 -o /dev/null -w '%{http_code}\n' \
    "$control_url/" 2>/dev/null || true

printf '\nOpenBao status:\n'
docker compose \
    --project-name "$compose_project" \
    --file "$compose_file" \
    exec -T -e BAO_ADDR=http://openbao:8200 openbao bao status -format=json 2>/dev/null \
    | sed -E 's/("(root_token|client_token|secret_id|unseal_keys_b64)"[[:space:]]*:[[:space:]]*)"[^"]*"/\1"<redacted>"/g' \
    || printf '%s\n' 'OpenBao is not running.'

printf 'Broker socket: '
if docker compose \
    --project-name "$compose_project" \
    --file "$compose_file" \
    exec -T arqen-broker test -S /run/arqen/gmail-broker.sock \
    >/dev/null 2>&1; then
    printf '%s\n' 'ready'
else
    printf '%s\n' 'not ready'
fi

token="$(arqen_read_token)"
base_url="http://127.0.0.1:${ARQEN_MCP_HOST_PORT:-8787}"
printf '\nMCP liveness: '
curl -sS --max-time 2 -o /dev/null -w '%{http_code}\n' \
    -H "Authorization: Bearer $token" "$base_url/healthz" 2>/dev/null || true
printf 'MCP readiness: '
curl -sS --max-time 2 -o /dev/null -w '%{http_code}\n' \
    -H "Authorization: Bearer $token" "$base_url/readyz" 2>/dev/null || true
