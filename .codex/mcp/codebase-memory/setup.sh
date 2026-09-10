#!/usr/bin/env bash
set -euo pipefail

runtime_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(git -C "$runtime_dir" rev-parse --show-toplevel)"
image="arqen-codebase-memory:0.10.8"

if ! command -v docker >/dev/null 2>&1; then
    echo "Arqen codebase-memory setup requires Docker." >&2
    exit 127
fi

docker build --network=host --tag "$image" "$runtime_dir"

version="$(docker run --rm --network none "$image" --version)"
if [[ "$version" != "codebase-memory-mcp 0.10.8" ]]; then
    echo "Unexpected codebase-memory version: $version" >&2
    exit 1
fi

bash "$runtime_dir/run.sh" config set auto_index true
bash "$runtime_dir/run.sh" config set auto_watch true
bash "$runtime_dir/run.sh" cli index_repository \
    --repo-path /workspace/project \
    --persistence false

echo "Built and indexed isolated Arqen codebase-memory image: $image"
echo "Repository: $root"
