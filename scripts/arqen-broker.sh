#!/usr/bin/env bash
set -euo pipefail

script_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/common.sh
. "$script_directory/lib/common.sh"
arqen_load_defaults
arqen_prepare_runtime
arqen_require_google_client

usurp=false
broker_arguments=()
for argument in "$@"; do
    case "$argument" in
        --usurp|--take-over)
            usurp=true
            ;;
        --help|-h)
            cat <<'EOF'
Usage: scripts/arqen-broker.sh [--usurp]

Starts the host credential broker. If an existing socket is stale, the script
asks before taking it over. --usurp accepts that takeover without prompting.
An active broker is never overwritten.
EOF
            exit 0
            ;;
        *)
            broker_arguments+=("$argument")
            ;;
    esac
done

broker_socket="$ARQEN_GMAIL_BROKER_SOCKET"
broker_started=false
cleanup_socket() {
    local status="$?"
    trap - EXIT INT TERM
    if [[ "$broker_started" == true ]] && [[ -S "$broker_socket" ]] && [[ "$(arqen_socket_state "$broker_socket")" == stale ]]; then
        unlink "$broker_socket" 2>/dev/null || true
    fi
    exit "$status"
}
trap cleanup_socket EXIT INT TERM

socket_state="$(arqen_socket_state "$broker_socket")"
case "$socket_state" in
    absent)
        ;;
    active)
        arqen_die "a credential broker is already listening at $broker_socket; stop that broker before starting another (--usurp cannot replace an active broker)"
        ;;
    other)
        arqen_die "cannot use $broker_socket because it is not a Unix socket; move the existing path before starting the broker"
        ;;
    unknown)
        arqen_die "cannot safely determine whether a broker owns $broker_socket; install ss or fuser, then retry"
        ;;
    stale)
        if [[ "$usurp" == true ]]; then
            printf 'Taking over stale credential broker socket: %s\n' "$broker_socket"
        elif [[ ! -t 0 || ! -t 1 ]]; then
            arqen_die "a stale credential broker socket exists at $broker_socket; rerun with --usurp from a trusted terminal to take it over"
        else
            printf 'A previous broker left a stale socket at %s.\n' "$broker_socket"
            printf 'No listener was detected, so it can be taken over. Take over this socket? [y/N] '
            answer=''
            IFS= read -r answer || true
            case "$answer" in
                y|Y|yes|YES|Yes)
                    printf 'Taking over stale credential broker socket.\n'
                    ;;
                *)
                    arqen_die "kept the existing socket; broker startup cancelled"
                    ;;
            esac
        fi
        unlink "$broker_socket" 2>/dev/null || arqen_die "could not remove stale broker socket at $broker_socket"
        ;;
    *)
        arqen_die "unrecognized broker socket state: $socket_state"
        ;;
esac

broker_started=true
arqen_cargo run --locked -- credential-broker "${broker_arguments[@]}"
