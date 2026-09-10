#!/usr/bin/env bash
set -euo pipefail

runtime_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(git -C "$runtime_dir" rev-parse --show-toplevel)"
root="$(realpath "$root")"
state_dir="$runtime_dir/state"
image="arqen-codebase-memory:0.10.8"

if ! command -v docker >/dev/null 2>&1; then
    echo "Arqen codebase-memory requires Docker; no host-global fallback is permitted." >&2
    exit 127
fi

if ! docker image inspect "$image" >/dev/null 2>&1; then
    echo "Arqen codebase-memory image is missing; run 'bash .codex/mcp/codebase-memory/setup.sh'." >&2
    exit 1
fi

mkdir -p "$state_dir/graph" "$state_dir/config" "$state_dir/home" "$state_dir/runtime"
chmod 700 "$state_dir" "$state_dir/graph" "$state_dir/config" "$state_dir/home" "$state_dir/runtime"

exec docker run --rm -i \
    --network none \
    --read-only \
    --tmpfs /tmp:rw,noexec,nosuid,nodev \
    --cap-drop=ALL \
    --security-opt=no-new-privileges \
    --user "$(id -u):$(id -g)" \
    --workdir /workspace/project \
    --mount "type=bind,src=$root,dst=/workspace/project,readonly" \
    --mount "type=bind,src=$state_dir,dst=/state" \
    --env HOME=/state/home \
    --env XDG_CONFIG_HOME=/state/config \
    --env XDG_RUNTIME_DIR=/state/runtime \
    --env CBM_CACHE_DIR=/state/graph \
    --env CBM_ALLOWED_ROOT=/workspace/project \
    "$image" "$@"
