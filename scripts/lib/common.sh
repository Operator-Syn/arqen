#!/usr/bin/env bash
set -euo pipefail

# Shared environment and command helpers for the named local workflows.
# shellcheck shell=bash

ARQEN_SCRIPT_DIRECTORY="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ARQEN_PROJECT_ROOT="$(cd "$ARQEN_SCRIPT_DIRECTORY/../.." && pwd)"

user_env_file="${XDG_CONFIG_HOME:-${HOME:-}/.config}/arqen/arqen.env"
if [[ -f "$user_env_file" ]]; then
    set -a
    # shellcheck disable=SC1090
    . "$user_env_file"
    set +a
fi

if [[ -f "$ARQEN_PROJECT_ROOT/.env" ]]; then
    set -a
    # shellcheck disable=SC1091
    . "$ARQEN_PROJECT_ROOT/.env"
    set +a
fi

readonly ARQEN_ROOT="$ARQEN_PROJECT_ROOT"
export ARQEN_ROOT
cd "$ARQEN_ROOT"

arqen_die() {
    printf 'arqen: %s\n' "$*" >&2
    exit 1
}

arqen_require_command() {
    command -v "$1" >/dev/null 2>&1 || arqen_die "required command not found: $1"
}

arqen_resolve_path() {
    local configured_path="${1:?path is required}"
    if [[ "$configured_path" == /* ]]; then
        printf '%s\n' "$configured_path"
    else
        printf '%s/%s\n' "$ARQEN_ROOT" "$configured_path"
    fi
}

arqen_load_defaults() {
    : "${ARQEN_MCP_LISTEN_ADDR:=127.0.0.1:8787}"
    : "${ARQEN_MCP_HOST_PORT:=8787}"
    : "${ARQEN_MCP_ALLOWED_HOSTS:=127.0.0.1:${ARQEN_MCP_HOST_PORT}}"
    : "${ARQEN_MCP_ALLOWED_ORIGINS:=http://127.0.0.1:${ARQEN_MCP_HOST_PORT}}"
    : "${ARQEN_MCP_BEARER_TOKEN_FILE:=${XDG_CONFIG_HOME:-${HOME:-}/.config}/arqen/mcp-bearer-token}"
    : "${ARQEN_COMPOSE_PROJECT:=arqen-local}"
    : "${ARQEN_USE_NIX:=auto}"
    export ARQEN_MCP_LISTEN_ADDR ARQEN_MCP_HOST_PORT ARQEN_MCP_ALLOWED_HOSTS
    export ARQEN_MCP_ALLOWED_ORIGINS ARQEN_MCP_BEARER_TOKEN_FILE
    export ARQEN_COMPOSE_PROJECT ARQEN_USE_NIX
}

arqen_prepare_runtime() {
    if [[ -z "${XDG_RUNTIME_DIR:-}" ]]; then
        local generated_runtime="${TMPDIR:-/tmp}/arqen-runtime-$(id -u)"
        mkdir -p "$generated_runtime"
        chmod 700 "$generated_runtime"
        export XDG_RUNTIME_DIR="$generated_runtime"
    fi
    [[ "$XDG_RUNTIME_DIR" == /* ]] || arqen_die "XDG_RUNTIME_DIR must be an absolute path"
    [[ -d "$XDG_RUNTIME_DIR" ]] || arqen_die "XDG_RUNTIME_DIR does not exist: $XDG_RUNTIME_DIR"

    if [[ -z "${ARQEN_GMAIL_BROKER_SOCKET:-}" ]]; then
        export ARQEN_GMAIL_BROKER_SOCKET="$XDG_RUNTIME_DIR/arqen/gmail-broker.sock"
    elif [[ "$ARQEN_GMAIL_BROKER_SOCKET" != /* ]]; then
        export ARQEN_GMAIL_BROKER_SOCKET="$(arqen_resolve_path "$ARQEN_GMAIL_BROKER_SOCKET")"
    fi
}

arqen_detect_docker_display() {
    local requested_mode="${ARQEN_DOCKER_DISPLAY_MODE:-auto}"
    local mode=""
    local wayland_socket=""
    local display_value=""
    local display_number=""
    local xauthority=""

    case "$requested_mode" in
        auto|wayland|x11)
            ;;
        *)
            arqen_die "ARQEN_DOCKER_DISPLAY_MODE must be auto, wayland, or x11"
            ;;
    esac

    if [[ "$requested_mode" != x11 && -n "${WAYLAND_DISPLAY:-}" ]]; then
        wayland_socket="$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY"
        if [[ -S "$wayland_socket" ]]; then
            mode=wayland
        elif [[ "$requested_mode" == wayland ]]; then
            arqen_die "Wayland socket is not available at $wayland_socket"
        fi
    elif [[ "$requested_mode" == wayland ]]; then
        arqen_die 'ARQEN_DOCKER_DISPLAY_MODE=wayland requires WAYLAND_DISPLAY'
    fi

    if [[ -z "$mode" && "$requested_mode" != wayland ]]; then
        display_value="${DISPLAY:-}"
        case "$display_value" in
            unix/:*)
                display_value="${display_value#unix/}"
                ;;
            unix:*)
                display_value="${display_value#unix}"
                ;;
        esac
        if [[ "$display_value" =~ ^:([0-9]+)(\..*)?$ ]]; then
            display_number="${BASH_REMATCH[1]}"
            if [[ -d /tmp/.X11-unix && -S "/tmp/.X11-unix/X$display_number" ]]; then
                xauthority="${XAUTHORITY:-${HOME:-}/.Xauthority}"
                if [[ -r "$xauthority" ]]; then
                    mode=x11
                elif [[ "$requested_mode" == x11 ]]; then
                    arqen_die "Xauthority file is not readable: $xauthority"
                fi
            elif [[ "$requested_mode" == x11 ]]; then
                arqen_die "X11 socket is not available at /tmp/.X11-unix/X$display_number"
            fi
        elif [[ "$requested_mode" == x11 ]]; then
            arqen_die "local X11 DISPLAY is required (got ${DISPLAY:-unset})"
        fi
    fi

    case "$mode" in
        wayland)
            export ARQEN_DOCKER_DISPLAY_MODE=wayland
            export ARQEN_DOCKER_WAYLAND_SOCKET="$wayland_socket"
            export ARQEN_DOCKER_DISPLAY_COMPOSE_FILE="$ARQEN_ROOT/deploy/containers/docker-native-compose-wayland.yml"
            ;;
        x11)
            export ARQEN_DOCKER_DISPLAY_MODE=x11
            export XAUTHORITY="$xauthority"
            export ARQEN_DOCKER_DISPLAY_COMPOSE_FILE="$ARQEN_ROOT/deploy/containers/docker-native-compose-x11.yml"
            ;;
        *)
            arqen_die 'no native display is available; run make docker-up from a Wayland or X11 desktop session'
            ;;
    esac

    export ARQEN_DOCKER_HOST_UID="${ARQEN_DOCKER_HOST_UID:-$(id -u)}"
    export ARQEN_DOCKER_HOST_GID="${ARQEN_DOCKER_HOST_GID:-$(id -g)}"
    [[ "$ARQEN_DOCKER_HOST_UID" =~ ^[0-9]+$ ]] \
        || arqen_die "ARQEN_DOCKER_HOST_UID must be numeric"
    [[ "$ARQEN_DOCKER_HOST_GID" =~ ^[0-9]+$ ]] \
        || arqen_die "ARQEN_DOCKER_HOST_GID must be numeric"
}

arqen_require_google_client() {
    local configured_path="${GOOGLE_CLIENT_SECRET:-${XDG_CONFIG_HOME:-${HOME:-}/.config}/arqen/google-client-secret.json}"
    local resolved_path
    resolved_path="$(arqen_resolve_path "$configured_path")"
    [[ -f "$resolved_path" ]] || arqen_die "Google OAuth client JSON not found at $resolved_path; add it or set GOOGLE_CLIENT_SECRET in .env"
    [[ -r "$resolved_path" ]] || arqen_die "Google OAuth client JSON is not readable: $resolved_path"
    export GOOGLE_CLIENT_SECRET="$resolved_path"
}

arqen_require_token() {
    if [[ -n "${ARQEN_MCP_BEARER_TOKEN:-}" ]]; then
        [[ ! "$ARQEN_MCP_BEARER_TOKEN" =~ [[:cntrl:]] ]] || arqen_die "ARQEN_MCP_BEARER_TOKEN contains a control character"
        return
    fi

    local configured_path="${ARQEN_MCP_BEARER_TOKEN_FILE:-}"
    [[ -n "$configured_path" ]] || arqen_die "set ARQEN_MCP_BEARER_TOKEN_FILE or ARQEN_MCP_BEARER_TOKEN in .env"
    local resolved_path
    resolved_path="$(arqen_resolve_path "$configured_path")"
    [[ -f "$resolved_path" && -r "$resolved_path" && -s "$resolved_path" ]] || arqen_die "MCP bearer token file is missing or empty: $resolved_path; run make setup-local"
    export ARQEN_MCP_BEARER_TOKEN_FILE="$resolved_path"
}

arqen_require_compose_token_file() {
    local configured_path="${ARQEN_MCP_BEARER_TOKEN_FILE:-}"
    [[ -n "$configured_path" ]] || arqen_die "Compose requires ARQEN_MCP_BEARER_TOKEN_FILE; run make setup-local"
    local resolved_path
    resolved_path="$(arqen_resolve_path "$configured_path")"
    [[ -f "$resolved_path" && -r "$resolved_path" && -s "$resolved_path" ]] || arqen_die "MCP bearer token file is missing or empty: $resolved_path; run make setup-local"
    export ARQEN_MCP_BEARER_TOKEN_FILE="$resolved_path"
}

arqen_socket_state() {
    local socket_path="${1:?socket path is required}"
    if [[ ! -e "$socket_path" ]]; then
        printf 'absent\n'
        return 0
    fi
    if [[ ! -S "$socket_path" ]]; then
        printf 'other\n'
        return 0
    fi

    local fuser_status=127
    if command -v fuser >/dev/null 2>&1; then
        fuser -s "$socket_path" >/dev/null 2>&1 || fuser_status="$?"
        if [[ "$fuser_status" -eq 0 ]]; then
            printf 'active\n'
            return 0
        fi
    fi

    if command -v ss >/dev/null 2>&1; then
        if ss -xlH 2>/dev/null | grep -F -- "$socket_path" >/dev/null; then
            printf 'active\n'
        elif [[ "$fuser_status" -eq 1 || "$fuser_status" -eq 127 ]]; then
            printf 'stale\n'
        else
            printf 'unknown\n'
        fi
        return 0
    fi

    if [[ "$fuser_status" -eq 1 ]]; then
        printf 'stale\n'
    else
        printf 'unknown\n'
    fi
}

arqen_read_token() {
    if [[ -n "${ARQEN_MCP_BEARER_TOKEN:-}" ]]; then
        printf '%s' "$ARQEN_MCP_BEARER_TOKEN"
    else
        cat "$ARQEN_MCP_BEARER_TOKEN_FILE"
    fi
}

arqen_cargo() {
    case "${ARQEN_USE_NIX:-auto}" in
        never)
            arqen_require_command cargo
            cargo "$@"
            ;;
        always)
            arqen_require_command nix
            nix develop "$ARQEN_ROOT#arqen" -c cargo "$@"
            ;;
        auto)
            if command -v nix >/dev/null 2>&1; then
                nix develop "$ARQEN_ROOT#arqen" -c cargo "$@"
            else
                arqen_require_command cargo
                cargo "$@"
            fi
            ;;
        *)
            arqen_die "ARQEN_USE_NIX must be auto, always, or never"
            ;;
    esac
}

arqen_require_docker_compose() {
    arqen_require_command docker
    docker compose version >/dev/null 2>&1 || arqen_die "Docker Compose v2 is required"
}

arqen_stop_pid() {
    local process_id="${1:-}"
    local attempt
    [[ "$process_id" =~ ^[0-9]+$ ]] || return 0
    kill -TERM "$process_id" 2>/dev/null || true
    for ((attempt = 0; attempt < 50; attempt++)); do
        if ! kill -0 "$process_id" 2>/dev/null; then
            wait "$process_id" 2>/dev/null || true
            return 0
        fi
        sleep 0.02
    done
    kill -KILL "$process_id" 2>/dev/null || true
    wait "$process_id" 2>/dev/null || true
}

arqen_stop_process_group() {
    local process_id="${1:-}"
    local attempt
    [[ "$process_id" =~ ^[0-9]+$ ]] || return 0
    kill -TERM -- "-$process_id" 2>/dev/null || arqen_stop_pid "$process_id"
    for ((attempt = 0; attempt < 50; attempt++)); do
        if ! kill -0 "$process_id" 2>/dev/null; then
            wait "$process_id" 2>/dev/null || true
            return 0
        fi
        sleep 0.02
    done
    kill -KILL -- "-$process_id" 2>/dev/null || kill -KILL "$process_id" 2>/dev/null || true
    wait "$process_id" 2>/dev/null || true
}
