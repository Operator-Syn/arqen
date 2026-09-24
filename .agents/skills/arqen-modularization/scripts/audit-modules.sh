#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
set -euo pipefail

repo_root="${1:-$(git rev-parse --show-toplevel)}"
src_root="$repo_root/src"
recommended_lines="${ARQEN_RECOMMENDED_MODULE_LINES:-${ARQEN_MODULE_MAX_LINES:-200}}"

if [[ ! -d "$src_root" ]]; then
    printf 'error: source directory not found: %s\n' "$src_root" >&2
    exit 1
fi
if [[ ! "$recommended_lines" =~ ^[0-9]+$ ]] || ((recommended_lines == 0)); then
    printf 'error: ARQEN_RECOMMENDED_MODULE_LINES must be a positive integer\n' >&2
    exit 1
fi

printf 'Arqen module audit\n'
printf 'root: %s\n' "$repo_root"
printf 'recommended review marker: %s lines (not a gate)\n\n' "$recommended_lines"

large_count=0
while IFS= read -r -d '' file; do
    lines="$(wc -l < "$file")"
    relative="${file#"$repo_root"/}"
    if ((lines > recommended_lines)); then
        printf 'review-signal: %4d %s\n' "$lines" "$relative"
        large_count=$((large_count + 1))
    fi
done < <(find "$src_root" -type f -name '*.rs' ! -name '*tests.rs' -print0 | sort -z)

if ((large_count == 0)); then
    printf 'production review signals: none\n'
else
    printf 'production review signals: %d\n' "$large_count"
fi

printf '\nfunction definitions for manual complexity review:\n'
rg -n --glob '*.rs' --glob '!**/*tests.rs' '^\s*(pub\s*\(crate\)\s+|pub\s+)?(async\s+)?fn\s+' "$src_root" || true
