#!/usr/bin/env bash
set -euo pipefail

script_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/common.sh
. "$script_directory/lib/common.sh"
arqen_load_defaults
arqen_require_command curl
arqen_require_command openssl

show_help() {
    cat <<'EOF'
Usage: scripts/arqen-smoke-local.sh [--call]
       scripts/arqen-smoke-local.sh --container [--call]

Starts an isolated broker and MCP process, then checks the authenticated
health endpoint and tools/list. --call additionally invokes live Gmail.
EOF
}

container_mode=false
live_call=false
for argument in "$@"; do
    case "$argument" in
        --container)
            container_mode=true
            ;;
        --call)
            live_call=true
            ;;
        --help|-h)
            show_help
            exit 0
            ;;
        *)
            arqen_die "unknown smoke option: $argument (use --help)"
            ;;
    esac
done

if [[ "$container_mode" == true ]]; then
    arqen_require_docker_compose
fi

smoke_runtime_dir="$(mktemp -d "${TMPDIR:-/tmp}/arqen-smoke-runtime.XXXXXX")"
chmod 700 "$smoke_runtime_dir"
broker_pid=""
mcp_pid=""
compose_file="$ARQEN_ROOT/deploy/containers/docker-compose.yml"
compose_project="$ARQEN_COMPOSE_PROJECT-smoke-$$"

cleanup() {
    local status="$?"
    trap - EXIT INT TERM
    if [[ "$container_mode" == true ]]; then
        docker compose --project-name "$compose_project" --file "$compose_file" down --remove-orphans >/dev/null 2>&1 || true
    fi
    if [[ -n "$mcp_pid" ]]; then
        arqen_stop_pid "$mcp_pid"
    fi
    if [[ -n "$broker_pid" ]]; then
        arqen_stop_pid "$broker_pid"
    fi
    rm -rf -- "$smoke_runtime_dir"
    exit "$status"
}
trap cleanup EXIT INT TERM

export XDG_RUNTIME_DIR="$smoke_runtime_dir"
export ARQEN_GMAIL_BROKER_SOCKET="$smoke_runtime_dir/arqen/gmail-broker.sock"

token_file="$smoke_runtime_dir/mcp-bearer-token"
umask 077
openssl rand -hex 32 > "$token_file"
chmod 600 "$token_file"
unset ARQEN_MCP_BEARER_TOKEN
export ARQEN_MCP_BEARER_TOKEN_FILE="$token_file"

if [[ "$live_call" == true ]]; then
    arqen_require_google_client
else
    # The broker only opens the client JSON when a live Gmail call is made.
    # Use an ephemeral placeholder for the no-network protocol smoke test.
    smoke_client="$smoke_runtime_dir/google-client-secret.json"
    printf '{}\n' > "$smoke_client"
    chmod 600 "$smoke_client"
    export GOOGLE_CLIENT_SECRET="$smoke_client"
fi

if [[ "$container_mode" == true ]]; then
    smoke_host_port="${ARQEN_MCP_SMOKE_HOST_PORT:-18788}"
    smoke_base_url="http://127.0.0.1:$smoke_host_port"
    export ARQEN_MCP_LISTEN_ADDR="0.0.0.0:8787"
    export ARQEN_MCP_HOST_PORT="$smoke_host_port"
    export ARQEN_MCP_ALLOWED_HOSTS="127.0.0.1:$smoke_host_port"
    export ARQEN_MCP_ALLOWED_ORIGINS="http://127.0.0.1:$smoke_host_port"
    export ARQEN_CONTAINER_UID="$(id -u)"
    export ARQEN_CONTAINER_GID="$(id -g)"
else
    smoke_listen_addr="${ARQEN_MCP_SMOKE_LISTEN_ADDR:-127.0.0.1:18787}"
    [[ "$smoke_listen_addr" == *:* ]] || arqen_die "ARQEN_MCP_SMOKE_LISTEN_ADDR must include a port"
    smoke_port="${smoke_listen_addr##*:}"
    smoke_base_url="http://127.0.0.1:$smoke_port"
    export ARQEN_MCP_LISTEN_ADDR="$smoke_listen_addr"
    export ARQEN_MCP_ALLOWED_HOSTS="127.0.0.1:$smoke_port"
    export ARQEN_MCP_ALLOWED_ORIGINS="http://127.0.0.1:$smoke_port"
fi

arqen_cargo build --locked
arqen_binary="$ARQEN_ROOT/target/debug/arqen"
[[ -x "$arqen_binary" ]] || arqen_die "cargo build did not produce $arqen_binary"

"$arqen_binary" credential-broker > "$smoke_runtime_dir/broker.log" 2>&1 &
broker_pid="$!"
for ((attempt = 0; attempt < 120; attempt++)); do
    if [[ -S "$ARQEN_GMAIL_BROKER_SOCKET" ]]; then
        break
    fi
    if ! kill -0 "$broker_pid" 2>/dev/null; then
        arqen_die "credential broker exited before creating its socket"
    fi
    sleep 0.1
done
[[ -S "$ARQEN_GMAIL_BROKER_SOCKET" ]] || arqen_die "credential broker did not create its socket"

if [[ "$container_mode" == true ]]; then
    arqen_require_docker_compose
    docker compose --project-name "$compose_project" --file "$compose_file" up --build -d
else
    "$arqen_binary" mcp-server > "$smoke_runtime_dir/mcp.log" 2>&1 &
    mcp_pid="$!"
fi

token="$(arqen_read_token)"
health_status=""
for ((attempt = 0; attempt < 120; attempt++)); do
    health_status="$(curl -sS --max-time 2 -o /dev/null -w '%{http_code}' \
        -H "Authorization: Bearer $token" "$smoke_base_url/healthz" 2>/dev/null || true)"
    if [[ "$health_status" == 204 ]]; then
        break
    fi
    if [[ "$container_mode" == false ]] && ! kill -0 "$mcp_pid" 2>/dev/null; then
        arqen_die "MCP server exited before its health endpoint became ready"
    fi
    sleep 0.1
done
[[ "$health_status" == 204 ]] || arqen_die "authenticated MCP health check did not return HTTP 204"

rpc_headers=(
    -H "Authorization: Bearer $token"
    -H 'Content-Type: application/json'
    -H 'Accept: application/json, text/event-stream'
    -H "Origin: $ARQEN_MCP_ALLOWED_ORIGINS"
)
tools_response="$(curl -sS --fail --max-time 5 "${rpc_headers[@]}" \
    --data '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' \
    "$smoke_base_url/mcp")" || arqen_die "authenticated MCP tools/list request failed"
[[ "$tools_response" == *'"list_emails"'* ]] || arqen_die "MCP tools/list did not advertise list_emails"

if [[ "$live_call" == true ]]; then
    call_response_file="$smoke_runtime_dir/list-emails.json"
    call_status="$(curl -sS --max-time 35 -o "$call_response_file" -w '%{http_code}' "${rpc_headers[@]}" \
        --data '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_emails","arguments":{"query":"in:inbox","max_results":5}}}' \
        "$smoke_base_url/mcp" 2>/dev/null || true)"
    [[ "$call_status" == 200 ]] || arqen_die "live list_emails request failed; verify the selected account, keyring, and Google access"
    if ! grep -q '"target_email"' "$call_response_file"; then
        arqen_die "live list_emails response did not contain an email result"
    fi
fi

if [[ "$container_mode" == true ]]; then
    if [[ "$live_call" == true ]]; then
        printf 'Container localhost smoke passed, including one live list_emails call.\n'
    else
        printf 'Container localhost smoke passed (protocol checks only; no Google request made).\n'
    fi
else
    if [[ "$live_call" == true ]]; then
        printf 'Native localhost smoke passed, including one live list_emails call.\n'
    else
        printf 'Native localhost smoke passed (protocol checks only; no Google request made).\n'
    fi
fi
