#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
set -euo pipefail

script_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/common.sh
. "$script_directory/lib/common.sh"
arqen_load_defaults
arqen_prepare_runtime
arqen_require_docker_compose

if [[ "${ARQEN_DOCKER_RESET_CONFIRM:-}" != YES ]]; then
    arqen_die "docker reset deletes the fresh Docker volumes; rerun with ARQEN_DOCKER_RESET_CONFIRM=YES"
fi

compose_file="$ARQEN_ROOT/deploy/containers/docker-native-compose.yml"
compose_project="${ARQEN_DOCKER_PROJECT:-arqen-docker}"
export ARQEN_ROOT
docker compose \
    --project-name "$compose_project" \
    --file "$compose_file" \
    down --volumes --remove-orphans

for generated_secret in \
    "$ARQEN_ROOT/.secrets/openbao-unseal-key" \
    "$ARQEN_ROOT/.secrets/openbao-bootstrap-token" \
    "$ARQEN_ROOT/.secrets/openbao-control-role-id" \
    "$ARQEN_ROOT/.secrets/openbao-control-secret-id" \
    "$ARQEN_ROOT/.secrets/openbao-broker-role-id" \
    "$ARQEN_ROOT/.secrets/openbao-broker-secret-id" \
    "$ARQEN_ROOT/.secrets/arqen-control-password"; do
    [[ -e "$generated_secret" ]] && unlink "$generated_secret"
done
printf '%s\n' 'Docker-native volumes and generated setup secrets were removed.'
