#!/usr/bin/env bash
set -euo pipefail

script_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/common.sh
. "$script_directory/lib/common.sh"
arqen_load_defaults
arqen_prepare_runtime
arqen_require_docker_compose
arqen_require_command curl
arqen_require_compose_token_file

broker_socket_state=""
for ((attempt = 0; attempt < 120; attempt++)); do
    broker_socket_state="$(arqen_socket_state "$ARQEN_GMAIL_BROKER_SOCKET")"
    [[ "$broker_socket_state" != absent ]] && break
    sleep 0.1
done
case "$broker_socket_state" in
    active)
        ;;
    absent)
        arqen_die "credential broker socket is not available at $ARQEN_GMAIL_BROKER_SOCKET; start make broker first"
        ;;
    stale)
        arqen_die "credential broker socket at $ARQEN_GMAIL_BROKER_SOCKET is stale; start make broker and confirm takeover, or use make broker ARQEN_BROKER_ARGS=--usurp"
        ;;
    other)
        arqen_die "credential broker path is not a Unix socket: $ARQEN_GMAIL_BROKER_SOCKET"
        ;;
    *)
        arqen_die "cannot safely determine whether the credential broker is listening at $ARQEN_GMAIL_BROKER_SOCKET"
        ;;
esac

export ARQEN_CONTAINER_UID="${ARQEN_CONTAINER_UID:-$(id -u)}"
export ARQEN_CONTAINER_GID="${ARQEN_CONTAINER_GID:-$(id -g)}"
export ARQEN_MCP_HOST_PORT
export ARQEN_MCP_ALLOWED_HOSTS ARQEN_MCP_ALLOWED_ORIGINS

compose_file="$ARQEN_ROOT/deploy/containers/docker-compose.yml"
docker compose \
    --project-name "$ARQEN_COMPOSE_PROJECT" \
    --file "$compose_file" \
    up --build -d "$@"

token="$(arqen_read_token)"
base_url="http://127.0.0.1:$ARQEN_MCP_HOST_PORT"
health_status=""
for ((attempt = 0; attempt < 120; attempt++)); do
    health_status="$(curl -sS --max-time 2 -o /dev/null -w '%{http_code}' \
        -H "Authorization: Bearer $token" "$base_url/healthz" 2>/dev/null || true)"
    if [[ "$health_status" == 204 ]]; then
        break
    fi
    sleep 0.5
done
[[ "$health_status" == 204 ]] || arqen_die "Docker MCP service did not become live at $base_url"
printf 'Docker MCP service is live at %s/mcp; readiness depends on the selected target.\n' "$base_url"
