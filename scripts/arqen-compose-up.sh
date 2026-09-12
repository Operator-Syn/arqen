#!/usr/bin/env bash
set -euo pipefail

script_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/common.sh
. "$script_directory/lib/common.sh"
arqen_load_defaults
arqen_prepare_runtime
arqen_require_docker_compose
arqen_require_compose_token_file

broker_socket_state="$(arqen_socket_state "$ARQEN_GMAIL_BROKER_SOCKET")"
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
exec docker compose \
    --project-name "$ARQEN_COMPOSE_PROJECT" \
    --file "$compose_file" \
    up --build -d "$@"
