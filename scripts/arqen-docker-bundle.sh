#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
set -euo pipefail

script_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/common.sh
. "$script_directory/lib/common.sh"
arqen_load_defaults

arqen_require_command docker
docker buildx version >/dev/null 2>&1 || arqen_die 'Docker Buildx is required to create OCI layouts'

arqen_require_command python3
metadata_script="$script_directory/arqen-docker-image-metadata.py"
source_metadata="$(python3 "$metadata_script" source-metadata --root "$ARQEN_ROOT")"
IFS=$'\t' read -r version source_commit source_state <<< "$source_metadata"
local_tag="${ARQEN_DOCKER_LOCAL_IMAGE_TAG:-dev}"
image_references="$(python3 "$metadata_script" references --tag "$local_tag")"
IFS=$'\t' read -r mcp_image runtime_image <<< "$image_references"

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
    local target="$1"
    local dockerfile="$2"
    local output_name="$3"
    local image_reference="$4"
    docker buildx build \
        --builder "$builder" \
        --platform linux/amd64 \
        --file "$ARQEN_ROOT/$dockerfile" \
        --target "$target" \
        --tag "$image_reference" \
        --label "org.opencontainers.image.version=$version" \
        --label "org.opencontainers.image.revision=$source_commit" \
        --label "com.operator-syn.arqen.source-state=$source_state" \
        --output "type=oci,dest=$stage_directory/$output_name.oci,tar=false" \
        --output "type=docker,dest=$stage_directory/$output_name.docker.tar" \
        "$ARQEN_ROOT"
    docker image load --input "$stage_directory/$output_name.docker.tar" >/dev/null
}

build_bundle_image mcp Dockerfile arqen-mcp "$mcp_image"
build_bundle_image runtime Dockerfile arqen-runtime "$runtime_image"
mcp_image_id="$(docker image inspect --format '{{.Id}}' "$mcp_image")"
runtime_image_id="$(docker image inspect --format '{{.Id}}' "$runtime_image")"

docker pull --platform linux/amd64 openbao/openbao:2.6.0 >/dev/null
docker pull --platform linux/amd64 debian:bookworm-slim >/dev/null
docker image save \
    --output "$stage_directory/docker-images-linux-amd64.tar" \
    "$mcp_image" \
    "$runtime_image" \
    openbao/openbao:2.6.0 \
    debian:bookworm-slim

python3 "$metadata_script" write-bundle-manifest \
    --root "$ARQEN_ROOT" \
    --tag "$local_tag" \
    --archive "$stage_directory/docker-images-linux-amd64.tar" \
    --manifest "$stage_directory/services.json" \
    --mcp-image-id "$mcp_image_id" \
    --runtime-image-id "$runtime_image_id"
python3 "$metadata_script" validate-bundle \
    --archive "$stage_directory/docker-images-linux-amd64.tar" \
    --manifest "$stage_directory/services.json" >/dev/null

mkdir -p "$output_directory"
rm -rf "$output_directory/arqen-mcp.oci" "$output_directory/arqen-runtime.oci"
cp -a "$stage_directory/arqen-mcp.oci" "$output_directory/"
cp -a "$stage_directory/arqen-runtime.oci" "$output_directory/"
install -m 0644 "$stage_directory/docker-images-linux-amd64.tar" \
    "$output_directory/docker-images-linux-amd64.tar"
install -m 0644 "$stage_directory/services.json" "$output_directory/services.json"
printf 'Docker bundle written to %s\n' "$output_directory"
