#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
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
export ARQEN_DOCKER_SECRETS_DIR="$ARQEN_ROOT/.secrets"
export ARQEN_DOCKER_HOST_UID ARQEN_DOCKER_HOST_GID

# Headless services may be restored by Docker while OpenBao remains sealed.
# Reuse the one-shot helper before starting the display-dependent control
# container; the unseal key stays mounted only in that helper.
docker compose "${compose_args[@]}" \
    --profile ops \
    run --rm openbao-unseal

# This session unit never builds or restarts the rest of the stack.
docker compose "${compose_args[@]}" \
    up --no-deps --no-build -d arqen-control
