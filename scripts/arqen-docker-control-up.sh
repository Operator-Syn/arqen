#!/usr/bin/env bash
set -euo pipefail

script_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/common.sh
. "$script_directory/lib/common.sh"
arqen_load_defaults
arqen_prepare_runtime
arqen_require_docker_compose
arqen_detect_docker_display
arqen_require_google_client
arqen_require_token

compose_file="$ARQEN_ROOT/deploy/containers/docker-native-compose.yml"
compose_project="${ARQEN_DOCKER_PROJECT:-arqen-docker}"
compose_args=(
    --project-name "$compose_project"
    --file "$compose_file"
    --file "$ARQEN_DOCKER_DISPLAY_COMPOSE_FILE"
)
export ARQEN_ROOT GOOGLE_CLIENT_SECRET ARQEN_MCP_BEARER_TOKEN_FILE
export ARQEN_DOCKER_SECRETS_DIR="${ARQEN_DOCKER_SECRETS_DIR:-$ARQEN_ROOT/.secrets}"
export ARQEN_DOCKER_HOST_UID ARQEN_DOCKER_HOST_GID

# Headless services are restored by Docker independently. This session unit
# only starts the display-dependent control container and never builds or
# restarts the rest of the stack.
docker compose "${compose_args[@]}" \
    up --no-deps --no-build -d arqen-control
