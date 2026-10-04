#!/bin/sh
set -eu

stack_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
export ARQEN_WAYLAND_UID="$(id -u)"
export ARQEN_WAYLAND_GID="$(id -g)"

if [ -n "${WAYLAND_DISPLAY:-}" ]; then
    case "$WAYLAND_DISPLAY" in
        /*) wayland_socket="$WAYLAND_DISPLAY" ;;
        *) wayland_socket="${XDG_RUNTIME_DIR:?XDG_RUNTIME_DIR is required}/$WAYLAND_DISPLAY" ;;
    esac
else
    wayland_socket="${XDG_RUNTIME_DIR:-/run/user/$ARQEN_WAYLAND_UID}/wayland-0"
fi
export ARQEN_WAYLAND_SOCKET="$wayland_socket"

compose() {
    docker compose \
        -f "$stack_dir/compose.yaml" \
        -f "$stack_dir/compose.wayland.yaml" \
        "$@"
}

print_credentials() {
    control_password=$(compose exec --user 0:0 -T arqen-control cat /run/arqen-control/control-password)
    bearer_token=$(compose exec --user 0:0 -T arqen-mcp cat /run/arqen-mcp/mcp-bearer-token)
    if [ -z "$control_password" ] || [ -z "$bearer_token" ]; then
        printf 'Arqen credentials are empty; check the OpenBao service.\n' >&2
        return 1
    fi
    control_address=$(compose port arqen-control 7681)
    mcp_address=$(compose port arqen-mcp 8787)
    printf '\nControl UI: http://%s\nUsername: arqen\nPassword: %s\n' \
        "$control_address" "$control_password"
    printf '\nMCP URL: http://%s/mcp\nBearer token: %s\n' \
        "$mcp_address" "$bearer_token"
}

case "${1:-}" in
    credentials)
        print_credentials
        exit
        ;;
    up)
        if [ ! -S "$ARQEN_WAYLAND_SOCKET" ]; then
            printf 'Wayland socket is not available: %s\n' "$ARQEN_WAYLAND_SOCKET" >&2
            exit 1
        fi
        # The one-shot permissions service updates shared volume metadata.
        # Stop writers first; Compose will recreate them with the override.
        compose stop arqen-control arqen-broker arqen-mcp
        ;;
esac

print_after_up=false
if [ "${1:-}" = up ]; then
    for argument in "$@"; do
        case "$argument" in
            -d|--detach) print_after_up=true ;;
        esac
    done
fi

compose "$@"
if [ "$print_after_up" = true ]; then
    print_credentials
fi
