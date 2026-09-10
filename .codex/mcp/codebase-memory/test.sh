#!/usr/bin/env bash
set -euo pipefail

runtime_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(git -C "$runtime_dir" rev-parse --show-toplevel)"
state_dir="$runtime_dir/state"
image="arqen-codebase-memory:0.10.8"
project="workspace-project"

fail() {
    echo "Arqen codebase-memory isolation check failed: $*" >&2
    exit 1
}

command -v docker >/dev/null 2>&1 || fail "Docker is unavailable."
docker image inspect "$image" >/dev/null 2>&1 || fail "image $image is missing; run setup.sh."

version="$(bash "$runtime_dir/run.sh" --version)"
[[ "$version" == "codebase-memory-mcp 0.10.8" ]] || fail "unexpected runtime version: $version"

for required in \
    '--network none' \
    '--read-only' \
    '--cap-drop=ALL' \
    '--security-opt=no-new-privileges' \
    'dst=/workspace/project,readonly' \
    'dst=/state' \
    'CBM_CACHE_DIR=/state/graph' \
    'CBM_ALLOWED_ROOT=/workspace/project'; do
    grep -Fq -- "$required" "$runtime_dir/run.sh" || fail "launcher is missing $required"
done

for directory in "$state_dir" "$state_dir/graph" "$state_dir/config" "$state_dir/home" "$state_dir/runtime"; do
    [[ -d "$directory" ]] || fail "missing state directory $directory"
    [[ "$(stat -c '%a' "$directory")" == "700" ]] || fail "state directory is not mode 700: $directory"
done

git -C "$root" check-ignore -q "$state_dir" || fail "runtime state is not ignored"
git -C "$root" check-ignore -q "$root/.secrets" || fail ".secrets is not ignored"
git -C "$root" check-ignore -q "$root/target" || fail "target is not ignored"

probe_dir="$(mktemp -d)"
trap 'rm -rf -- "$probe_dir"' EXIT

request_file="$probe_dir/request.ndjson"
response_file="$probe_dir/response.ndjson"
error_file="$probe_dir/stderr.log"

printf '%s\n' \
    '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"arqen-codebase-memory-test","version":"1.0.0"}}}' \
    '{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}' \
    '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
    >"$request_file"

if ! bash "$runtime_dir/run.sh" <"$request_file" >"$response_file" 2>"$error_file"; then
    fail "stdio MCP handshake failed: $(<"$error_file")"
fi

while IFS= read -r line; do
    case "$line" in
        "" | \{*\}) ;;
        *) fail "non-JSON output contaminated MCP stdout" ;;
    esac
done <"$response_file"

tools_response="$(grep -m1 '"id":2' "$response_file")"
[[ -n "$tools_response" ]] || fail "tools/list returned no response"
actual_tools="$(grep -o '"name":"[^"]*"' <<<"$tools_response" | sed 's/^"name":"//; s/"$//' | sort -u)"
expected_tools="$(printf '%s\n' \
    check_index_coverage \
    delete_project \
    detect_changes \
    get_architecture \
    get_code_snippet \
    get_graph_schema \
    index_repository \
    index_status \
    ingest_traces \
    list_projects \
    manage_adr \
    query_graph \
    search_code \
    search_graph \
    trace_path | sort -u)"
[[ "$actual_tools" == "$expected_tools" ]] || fail "unexpected MCP tool surface: $actual_tools"

bash "$runtime_dir/run.sh" cli list_projects >/dev/null
bash "$runtime_dir/run.sh" cli index_status --project "$project" >/dev/null
cargo_coverage=""
if ! cargo_coverage="$(bash "$runtime_dir/run.sh" cli check_index_coverage \
    --project "$project" --paths Cargo.toml)"; then
    fail "Cargo.toml index coverage query failed."
fi
grep -Fq '"status":"no_recorded_issue"' <<<"$cargo_coverage" \
    || fail "Cargo.toml index coverage is not clean: $cargo_coverage"
bash "$runtime_dir/run.sh" cli get_architecture --project "$project" --aspects overview >/dev/null
bash "$runtime_dir/run.sh" cli search_graph --project "$project" --name-pattern '.*main.*' --limit 5 >/dev/null
bash "$runtime_dir/run.sh" cli search_code --project "$project" --pattern 'fn main' --limit 5 >/dev/null

if bash "$runtime_dir/run.sh" cli index_repository --repo-path /workspace --persistence false \
    >"$probe_dir/out-of-root.stdout" 2>"$probe_dir/out-of-root.stderr"; then
    fail "out-of-root indexing unexpectedly succeeded"
fi

if (
    cd "$probe_dir"
    bash -lc 'set -euo pipefail; root="$(git rev-parse --show-toplevel 2>/dev/null)" || { echo "MCP launcher must run inside the target Git repository." >&2; exit 1; }; exec bash "$root/.codex/mcp/codebase-memory/run.sh"'
) >"$probe_dir/outside-git.stdout" 2>"$probe_dir/outside-git.stderr"; then
    fail "project registration launcher unexpectedly succeeded outside Git"
fi

echo "Arqen codebase-memory isolation checks passed (15 tools)."
