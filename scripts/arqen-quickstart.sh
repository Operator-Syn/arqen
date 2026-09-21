#!/usr/bin/env bash
set -euo pipefail

script_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(cd "$script_directory/.." && pwd)"

show_help() {
    cat <<'EOF'
Usage: scripts/arqen-quickstart.sh [--enable] [--enable-native-mcp] [--enable-linger] [--restart]

Builds and installs Arqen for the current user, prepares XDG configuration,
and installs the native systemd user services. Preparation is non-activating
by default.

  --enable          enable and start the host credential broker (Docker MCP path)
  --enable-native-mcp enable and start the optional native MCP service
  --enable-linger   enable systemd user lingering for boot/logout persistence
  --restart         restart services after installation when they are active
  --help            show this help
EOF
}

enable_broker=false
enable_native_mcp=false
enable_linger=false
restart_services=false
for argument in "$@"; do
    case "$argument" in
        --enable)
            enable_broker=true
            ;;
        --enable-broker)
            enable_broker=true
            ;;
        --enable-native-mcp)
            enable_native_mcp=true
            ;;
        --enable-linger)
            enable_linger=true
            ;;
        --restart)
            restart_services=true
            ;;
        --help|-h)
            show_help
            exit 0
            ;;
        *)
            printf 'arqen: unknown quickstart option: %s (use --help)\n' "$argument" >&2
            exit 1
            ;;
    esac
done

# shellcheck source=lib/common.sh
. "$script_directory/lib/common.sh"
arqen_load_defaults
arqen_require_command install
arqen_require_command mktemp
arqen_require_command systemctl

config_home="${XDG_CONFIG_HOME:-${HOME:-}/.config}"
[[ "$config_home" == /* ]] || arqen_die "XDG_CONFIG_HOME must be an absolute path"
[[ -n "${HOME:-}" ]] || arqen_die "HOME must be set for the user quickstart"
config_directory="$config_home/arqen"
bin_directory="$HOME/.local/bin"
unit_directory="$config_home/systemd/user"

mkdir -p "$config_directory" "$bin_directory" "$unit_directory"
chmod 700 "$config_directory"

printf 'Building the locked release binary...\n'
arqen_cargo build --release --locked

source_binary="$project_root/target/release/arqen"
[[ -x "$source_binary" ]] || arqen_die "release binary was not produced at $source_binary"
temporary_binary="$(mktemp "$bin_directory/.arqen.XXXXXX")"
cleanup() {
    rm -f -- "$temporary_binary"
}
trap cleanup EXIT
install -m 0755 "$source_binary" "$temporary_binary"
mv -f -- "$temporary_binary" "$bin_directory/arqen"
trap - EXIT

install_project_wrapper() {
    local wrapper_name="$1"
    local project_script="$2"
    local target="$bin_directory/$wrapper_name"
    local temporary_wrapper

    temporary_wrapper="$(mktemp "$bin_directory/.${wrapper_name}.XXXXXX")"
    if ! (
        umask 077
        printf '%s\n' '#!/usr/bin/env bash' 'set -euo pipefail'
        printf 'export ARQEN_PROJECT_ROOT=%q\n' "$project_root"
        printf 'exec %q "$@"\n' "$project_script"
    ) >"$temporary_wrapper"; then
        rm -f -- "$temporary_wrapper"
        return 1
    fi
    chmod 0755 "$temporary_wrapper"
    mv -f -- "$temporary_wrapper" "$target"
}

# Docker control lifecycle helpers need the repository scripts and compose
# files, while the user unit itself lives in the standard per-user systemd
# directory. Keep the repository root explicit in these small wrappers.
install_project_wrapper arqen-docker-control-up "$project_root/scripts/arqen-docker-control-up.sh"
install_project_wrapper arqen-docker-control-down "$project_root/scripts/arqen-docker-control-down.sh"

env_file="$config_directory/arqen.env"
token_file="$config_directory/mcp-bearer-token"
client_secret="$config_directory/google-client-secret.json"
if [[ ! -e "$env_file" ]]; then
    umask 077
    {
        printf 'GOOGLE_CLIENT_SECRET=%s\n' "$client_secret"
        printf 'ARQEN_MCP_BEARER_TOKEN_FILE=%s\n' "$token_file"
        printf 'ARQEN_MCP_ALLOWED_HOSTS=127.0.0.1:8787\n'
        printf 'ARQEN_MCP_ALLOWED_ORIGINS=http://127.0.0.1:8787\n'
    } >"$env_file"
    chmod 600 "$env_file"
fi
chmod 600 "$env_file"

if [[ ! -e "$token_file" ]]; then
    umask 077
    if command -v openssl >/dev/null 2>&1; then
        openssl rand -hex 32 >"$token_file"
    else
        head -c 32 /dev/urandom | base64 | tr -d '\n' >"$token_file"
        printf '\n' >>"$token_file"
    fi
    chmod 600 "$token_file"
fi
chmod 600 "$token_file"

install -m 0644 "$project_root/deploy/systemd/arqen-credential-broker.service" \
    "$unit_directory/arqen-credential-broker.service"
install -m 0644 "$project_root/deploy/systemd/arqen-mcp.service" \
    "$unit_directory/arqen-mcp.service"
install -m 0644 "$project_root/deploy/systemd/arqen-docker-control.service" \
    "$unit_directory/arqen-docker-control.service"
systemctl --user daemon-reload

if [[ "$enable_linger" == true ]]; then
    arqen_require_command loginctl
    loginctl enable-linger "$USER"
fi

if [[ "$enable_broker" == true ]]; then
    systemctl --user enable arqen-credential-broker.service
    if [[ "$restart_services" == true ]]; then
        systemctl --user restart arqen-credential-broker.service
    else
        systemctl --user start arqen-credential-broker.service
    fi
fi

if [[ "$enable_native_mcp" == true ]]; then
    systemctl --user enable arqen-mcp.service
    if [[ "$restart_services" == true ]]; then
        systemctl --user restart arqen-mcp.service
    else
        systemctl --user start arqen-mcp.service
    fi
fi

printf '\nArqen is prepared for the current user.\n'
printf '  Binary: %s\n' "$bin_directory/arqen"
printf '  Config: %s\n' "$env_file"
printf '  Token:  %s (value not printed)\n' "$token_file"
printf '  OAuth client: %s (provide this file before login)\n' "$client_secret"
printf '\nFor the Docker MCP VPS path, activate the host broker then start Compose:\n'
printf '  systemctl --user enable --now arqen-credential-broker.service\n'
printf '  make compose-up\n'
printf '\nFor the optional all-native MCP path instead:\n'
printf '  systemctl --user enable --now arqen-credential-broker.service arqen-mcp.service\n'
printf '  loginctl enable-linger %s\n' "$USER"
printf '\nFor the Docker-native Wayland control container after login:\n'
printf '  systemctl --user enable --now arqen-docker-control.service\n'
printf '\nFor remote OAuth, keep the SSH tunnel on port 8765 and run:\n'
printf '  ssh -t -L 127.0.0.1:8765:127.0.0.1:8765 %s@your-vps '\''ARQEN_OAUTH_REMOTE=1 ARQEN_OAUTH_CALLBACK_PORT=8765 %s/arqen'\''\n' "$USER" "$bin_directory"
