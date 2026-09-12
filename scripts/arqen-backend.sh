#!/usr/bin/env bash
set -euo pipefail

script_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(cd "$script_directory/.." && pwd)"

show_help() {
    cat <<'EOF'
Usage: scripts/arqen-backend.sh [--usurp]

Starts the host credential broker and the native MCP HTTP server as one local
backend. The TUI remains a separate frontend for login and target selection.

--usurp       take over a stale broker socket without prompting
--help        show this help
EOF
}

broker_arguments=()
for argument in "$@"; do
    case "$argument" in
        --usurp|--take-over)
            broker_arguments+=(--usurp)
            ;;
        --help|-h)
            show_help
            exit 0
            ;;
        *)
            printf 'arqen: unknown backend option: %s (use --help)\n' "$argument" >&2
            exit 1
            ;;
    esac
done

# A backend run is self-contained: create the ignored local configuration when
# it has not been initialized yet, but never overwrite an existing .env.
if [[ ! -e "$project_root/.env" ]]; then
    "$script_directory/arqen-setup-local.sh"
fi

# shellcheck source=lib/common.sh
. "$script_directory/lib/common.sh"
arqen_load_defaults
arqen_prepare_runtime
arqen_require_google_client
arqen_require_token
arqen_require_command curl
arqen_require_command setsid

broker_pid=""
mcp_pid=""
broker_socket="$ARQEN_GMAIL_BROKER_SOCKET"
backend_port="${ARQEN_MCP_LISTEN_ADDR##*:}"
backend_url="http://127.0.0.1:$backend_port"

if [[ "$backend_port" =~ ^[0-9]+$ ]] && command -v ss >/dev/null 2>&1; then
    if ss -ltnH 2>/dev/null | awk -v port="$backend_port" '$4 ~ (":" port "$") { found = 1 } END { exit found ? 0 : 1 }'; then
        arqen_die "MCP port $backend_port is already in use; stop the owning service or set ARQEN_MCP_LISTEN_ADDR, ARQEN_MCP_ALLOWED_HOSTS, and ARQEN_MCP_ALLOWED_ORIGINS to another local port in .env"
    fi
fi

cleanup() {
    local status="$?"
    trap - EXIT INT TERM
    if [[ -n "$mcp_pid" ]]; then
        arqen_stop_process_group "$mcp_pid"
    fi
    if [[ -n "$broker_pid" ]]; then
        arqen_stop_process_group "$broker_pid"
    fi
    exit "$status"
}
trap cleanup EXIT INT TERM

socket_state="$(arqen_socket_state "$broker_socket")"
case "$socket_state" in
    active)
        printf 'Reusing the already-running credential broker at %s\n' "$broker_socket"
        ;;
    absent|stale)
        setsid --wait "$script_directory/arqen-broker.sh" "${broker_arguments[@]}" &
        broker_pid="$!"
        for ((attempt = 0; attempt < 120; attempt++)); do
            if [[ "$(arqen_socket_state "$broker_socket")" == active ]]; then
                break
            fi
            if ! kill -0 "$broker_pid" 2>/dev/null; then
                wait "$broker_pid" 2>/dev/null || true
                arqen_die "credential broker did not start"
            fi
            sleep 0.1
        done
        [[ "$(arqen_socket_state "$broker_socket")" == active ]] || arqen_die "credential broker did not become ready"
        ;;
    other)
        arqen_die "credential broker path is not a Unix socket: $broker_socket"
        ;;
    *)
        arqen_die "cannot safely determine whether the credential broker is listening at $broker_socket"
        ;;
esac

setsid --wait "$script_directory/arqen-mcp.sh" &
mcp_pid="$!"

token="$(arqen_read_token)"
health_status=""
for ((attempt = 0; attempt < 120; attempt++)); do
    health_status="$(curl -sS --max-time 2 -o /dev/null -w '%{http_code}' \
        -H "Authorization: Bearer $token" "$backend_url/healthz" 2>/dev/null || true)"
    if [[ "$health_status" == 204 ]]; then
        break
    fi
    if ! kill -0 "$mcp_pid" 2>/dev/null; then
        wait "$mcp_pid" 2>/dev/null || true
        arqen_die "MCP server did not start; check the configured listen address and bearer settings"
    fi
    sleep 0.1
done
[[ "$health_status" == 204 ]] || arqen_die "MCP health check did not return HTTP 204"

printf 'Arqen backend is ready.\n'
printf '  MCP endpoint: %s/mcp\n' "$backend_url"
printf '  Broker socket: %s\n' "$broker_socket"
printf '  TUI target: select an eligible account and press t\n'
printf 'Press Ctrl-C to stop the MCP server and any broker started by this command.\n'

while :; do
    if ! kill -0 "$mcp_pid" 2>/dev/null; then
        wait "$mcp_pid" 2>/dev/null || true
        arqen_die "MCP server stopped unexpectedly"
    fi
    if [[ -n "$broker_pid" ]] && ! kill -0 "$broker_pid" 2>/dev/null; then
        wait "$broker_pid" 2>/dev/null || true
        arqen_die "credential broker stopped unexpectedly"
    fi
    sleep 1
done
