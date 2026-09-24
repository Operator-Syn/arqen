#!/usr/bin/env bash
set -euo pipefail

script_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/common.sh
. "$script_directory/lib/common.sh"
arqen_load_defaults

arqen_require_command docker
docker buildx version >/dev/null 2>&1 || arqen_die 'Docker Buildx is required to create OCI layouts'

version="$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["package"]["version"])')"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] \
    || arqen_die "Cargo.toml must contain a stable numeric version; found: $version"

output_directory="$ARQEN_ROOT/out/arqen-docker-stack"
stage_directory="$(mktemp -d "${TMPDIR:-/tmp}/arqen-docker-bundle.XXXXXX")"
builder="arqen-bundle-$(id -u)-$$"
builder_created=false
cleanup() {
    if [[ "$builder_created" == true ]]; then
        docker buildx rm "$builder" >/dev/null 2>&1 || true
    fi
    rm -rf "$stage_directory"
}
trap cleanup EXIT

docker buildx create --name "$builder" --driver docker-container \
    --driver-opt network=host >/dev/null
builder_created=true
docker buildx inspect --builder "$builder" --bootstrap >/dev/null

build_bundle_image() {
    local image_name="$1"
    local dockerfile="$2"
    local context="$3"
    local output_name="$4"
    docker buildx build \
        --builder "$builder" \
        --platform linux/amd64 \
        --file "$context/$dockerfile" \
        --tag "ghcr.io/operator-syn/$image_name:$version" \
        --tag "ghcr.io/operator-syn/$image_name:latest" \
        --output "type=oci,dest=$stage_directory/$output_name.oci,tar=false" \
        --output "type=docker,dest=$stage_directory/$output_name.docker.tar" \
        "$context"
    docker image load --input "$stage_directory/$output_name.docker.tar" >/dev/null
}

build_bundle_image arqen-mcp Dockerfile "$ARQEN_ROOT" arqen-mcp
build_bundle_image arqen-runtime Dockerfile.docker-native "$ARQEN_ROOT" arqen-runtime

docker pull --platform linux/amd64 openbao/openbao:2.6.0 >/dev/null
docker pull --platform linux/amd64 debian:bookworm-slim >/dev/null
docker image save \
    --output "$stage_directory/docker-images-linux-amd64.tar" \
    "ghcr.io/operator-syn/arqen-mcp:$version" \
    ghcr.io/operator-syn/arqen-mcp:latest \
    "ghcr.io/operator-syn/arqen-runtime:$version" \
    ghcr.io/operator-syn/arqen-runtime:latest \
    openbao/openbao:2.6.0 \
    debian:bookworm-slim

python3 - "$stage_directory/services.json" "$version" <<'PY'
import json
import sys

path, version = sys.argv[1:]
manifest = {
    "platform": "linux/amd64",
    "version": version,
    "services": {
        "arqen-broker": f"ghcr.io/operator-syn/arqen-runtime:{version}",
        "arqen-control": f"ghcr.io/operator-syn/arqen-runtime:{version}",
        "arqen-mcp": f"ghcr.io/operator-syn/arqen-mcp:{version}",
        "openbao": "openbao/openbao:2.6.0",
        "openbao-bootstrap": "openbao/openbao:2.6.0",
        "openbao-unseal": "openbao/openbao:2.6.0",
        "arqen-secret-init": "debian:bookworm-slim",
    },
    "docker_load_archive": "docker-images-linux-amd64.tar",
    "oci_layouts": ["arqen-mcp.oci", "arqen-runtime.oci"],
}
with open(path, "w", encoding="utf-8") as stream:
    json.dump(manifest, stream, indent=2)
    stream.write("\n")
PY

mkdir -p "$output_directory"
rm -rf "$output_directory/arqen-mcp.oci" "$output_directory/arqen-runtime.oci"
cp -a "$stage_directory/arqen-mcp.oci" "$output_directory/"
cp -a "$stage_directory/arqen-runtime.oci" "$output_directory/"
install -m 0644 "$stage_directory/docker-images-linux-amd64.tar" \
    "$output_directory/docker-images-linux-amd64.tar"
install -m 0644 "$stage_directory/services.json" "$output_directory/services.json"
printf 'Docker bundle written to %s\n' "$output_directory"
