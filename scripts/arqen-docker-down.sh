#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
set -euo pipefail

script_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/common.sh
. "$script_directory/lib/common.sh"
arqen_load_defaults
arqen_prepare_runtime
arqen_require_docker_compose

compose_file="$ARQEN_ROOT/deploy/containers/docker-native-compose.yml"
compose_project="${ARQEN_DOCKER_PROJECT:-arqen-docker}"
export ARQEN_ROOT
exec docker compose \
    --project-name "$compose_project" \
    --file "$compose_file" \
    down --remove-orphans "$@"
