#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
set -euo pipefail

script_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/common.sh
. "$script_directory/lib/common.sh"
arqen_load_defaults

run_check() {
    local label="$1"
    shift
    printf '== %s ==\n' "$label"
    "$@"
}

run_check 'shell script syntax' bash -n scripts/lib/common.sh scripts/*.sh
if command -v nix >/dev/null 2>&1; then
    run_check 'shellcheck OpenBao startup scripts' \
        nix develop "$ARQEN_ROOT#arqen" -c shellcheck -e SC1091 \
        deploy/containers/openbao/bootstrap.sh \
        scripts/arqen-docker-up.sh \
        scripts/arqen-openbao-smoke.sh \
        scripts/arqen-openbao-image-smoke.sh
elif command -v shellcheck >/dev/null 2>&1; then
    run_check 'shellcheck OpenBao startup scripts' shellcheck -e SC1091 \
        deploy/containers/openbao/bootstrap.sh \
        scripts/arqen-docker-up.sh \
        scripts/arqen-openbao-smoke.sh \
        scripts/arqen-openbao-image-smoke.sh
else
    printf '== shellcheck OpenBao startup scripts ==\nNOT RUN (install ShellCheck or Nix)\n'
fi
run_check 'cargo fmt --check' arqen_cargo fmt --check
run_check 'cargo test' arqen_cargo test --locked
run_check 'cargo clippy' arqen_cargo clippy --locked --all-targets --all-features -- -D warnings
run_check 'cargo build' arqen_cargo build --locked

if command -v nix >/dev/null 2>&1; then
    run_check 'nix flake check --no-build' nix flake check --no-build "$ARQEN_ROOT"
else
    printf '== nix flake check --no-build ==\nNOT RUN (nix is not installed)\n'
fi

run_check 'git diff --check' git diff --check
printf 'All available local checks passed.\n'
