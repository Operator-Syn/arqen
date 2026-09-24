#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
set -euo pipefail

hooks_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(git -C "$hooks_directory" rev-parse --show-toplevel)"

if [[ ! -x "$hooks_directory/pre-commit" ]]; then
    echo "Git hook is missing or not executable: $hooks_directory/pre-commit" >&2
    exit 1
fi

git -C "$repo_root" config --local core.hooksPath .githooks
configured_path="$(git -C "$repo_root" config --local --get core.hooksPath)"

if [[ "$configured_path" != ".githooks" ]]; then
    echo "Could not verify core.hooksPath (got: $configured_path)" >&2
    exit 1
fi

echo "Configured the one-file pre-commit hook for $repo_root"
