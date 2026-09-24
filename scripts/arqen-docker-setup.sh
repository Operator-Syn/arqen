#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
set -euo pipefail

script_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(cd "$script_directory/.." && pwd)"

if [[ ! -f "$project_root/.env" ]]; then
    "$script_directory/arqen-setup-local.sh"
fi

# shellcheck source=lib/common.sh
. "$script_directory/lib/common.sh"
arqen_load_defaults
arqen_prepare_runtime
arqen_require_docker_compose
arqen_require_command openssl
arqen_require_google_client
arqen_require_token

secrets_directory="$ARQEN_ROOT/.secrets"
mkdir -p "$secrets_directory"
chmod 700 "$secrets_directory"

create_secret() {
    local path="$1"
    local bytes="$2"
    if [[ ! -s "$path" ]]; then
        umask 077
        openssl rand -hex "$bytes" > "$path"
        printf 'Generated protected Docker secret: %s\n' "$path"
    else
        printf 'Kept existing protected Docker secret: %s\n' "$path"
    fi
    chmod 600 "$path"
}

create_secret "$secrets_directory/mcp-bearer-token" 32
create_secret "$secrets_directory/arqen-control-password" 24

compose_file="$ARQEN_ROOT/deploy/containers/docker-native-compose.yml"
compose_project="${ARQEN_DOCKER_PROJECT:-arqen-docker}"

export ARQEN_ROOT
export ARQEN_DOCKER_SECRETS_DIR="$secrets_directory"
export ARQEN_DOCKER_SECRET_UID="$(id -u)"
export ARQEN_DOCKER_SECRET_GID="$(id -g)"
export GOOGLE_CLIENT_SECRET
export ARQEN_MCP_BEARER_TOKEN_FILE
export ARQEN_MCP_ALLOWED_HOSTS ARQEN_MCP_ALLOWED_ORIGINS

printf 'Starting OpenBao and initializing the Docker-native profile...\n'
docker compose \
    --project-name "$compose_project" \
    --file "$compose_file" \
    up -d openbao
docker compose \
    --project-name "$compose_project" \
    --file "$compose_file" \
    --profile ops \
    run --rm openbao-bootstrap

printf '\nDocker-native setup is ready for make docker-up.\n'
printf '  TUI URL after startup: http://127.0.0.1:%s\n' "${ARQEN_CONTROL_HOST_PORT:-7681}"
printf '  MCP URL after startup: http://127.0.0.1:%s/mcp\n' "${ARQEN_MCP_HOST_PORT:-8787}"
printf '  TUI credentials: %s (value not printed)\n' "$secrets_directory/arqen-control-password"
