#!/usr/bin/env bash
set -euo pipefail

script_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(cd "$script_directory/.." && pwd)"

if [[ ! -e "$project_root/.env" ]]; then
    cp -- "$project_root/.env.example" "$project_root/.env"
    chmod 600 "$project_root/.env"
    printf 'Created %s\n' "$project_root/.env"
elif [[ ! -f "$project_root/.env" ]]; then
    printf 'arqen: .env exists but is not a regular file: %s\n' "$project_root/.env" >&2
    exit 1
fi

# shellcheck source=lib/common.sh
. "$script_directory/lib/common.sh"
arqen_load_defaults

secrets_directory="$ARQEN_ROOT/.secrets"
mkdir -p "$secrets_directory"
chmod 700 "$secrets_directory"

if [[ -z "${ARQEN_MCP_BEARER_TOKEN:-}" ]]; then
    token_file="$(arqen_resolve_path "$ARQEN_MCP_BEARER_TOKEN_FILE")"
    mkdir -p "$(dirname "$token_file")"
    chmod 700 "$(dirname "$token_file")"
    if [[ ! -s "$token_file" ]]; then
        arqen_require_command openssl
        umask 077
        openssl rand -hex 32 > "$token_file"
        printf 'Generated a user-only MCP bearer token at %s\n' "$token_file"
    else
        printf 'Kept the existing MCP bearer token at %s\n' "$token_file"
    fi
    chmod 600 "$token_file"
else
    printf 'Using ARQEN_MCP_BEARER_TOKEN from the ignored .env file\n'
fi

client_path="$(arqen_resolve_path "${GOOGLE_CLIENT_SECRET:-.secrets/google-client-secret.json}")"
if [[ -f "$client_path" ]]; then
    printf 'Google OAuth client JSON: found at %s\n' "$client_path"
else
    printf 'Google OAuth client JSON: add it at %s before using the TUI or live Gmail calls\n' "$client_path"
fi

printf '\nNext steps:\n'
printf '  make check\n'
printf '  make broker   # terminal A\n'
printf '  make mcp      # terminal B\n'
printf '  make smoke-local\n'
