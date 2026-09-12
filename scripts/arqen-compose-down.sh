#!/usr/bin/env bash
set -euo pipefail

script_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/common.sh
. "$script_directory/lib/common.sh"
arqen_load_defaults
arqen_prepare_runtime
arqen_require_docker_compose
arqen_require_compose_token_file

compose_file="$ARQEN_ROOT/deploy/containers/docker-compose.yml"
exec docker compose \
    --project-name "$ARQEN_COMPOSE_PROJECT" \
    --file "$compose_file" \
    down --remove-orphans "$@"
