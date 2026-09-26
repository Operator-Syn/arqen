#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Read Arqen Docker image metadata and validate local image bundles."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import tarfile
import tomllib
from pathlib import Path, PurePosixPath
from typing import Any


VERSION_PATTERN = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
TAG_PATTERN = re.compile(r"^[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}$")
IMAGE_ID_PATTERN = re.compile(r"^sha256:[0-9a-f]{64}$")
COMMIT_PATTERN = re.compile(r"^(?:[0-9a-f]{40}|[0-9a-f]{64})$")
LOCAL_NAMESPACE = "arqen-local"
BASE_SERVICES = {
    "openbao": "openbao/openbao:2.6.0",
    "openbao-bootstrap": "openbao/openbao:2.6.0",
    "openbao-unseal": "openbao/openbao:2.6.0",
    "arqen-secret-init": "debian:bookworm-slim",
}


def package_version(root: Path) -> str:
    try:
        with (root / "Cargo.toml").open("rb") as stream:
            version = tomllib.load(stream)["package"]["version"]
    except (OSError, KeyError, tomllib.TOMLDecodeError) as error:
        raise ValueError(f"could not read the root Cargo package version: {error}") from error
    if not isinstance(version, str) or not VERSION_PATTERN.fullmatch(version):
        raise ValueError(f"Cargo.toml must contain a stable X.Y.Z version; found: {version!r}")
    return version


def source_metadata(root: Path) -> dict[str, str]:
    version = package_version(root)
    try:
        commit_result = subprocess.run(
            ["git", "-C", str(root), "rev-parse", "--verify", "HEAD"],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return {"version": version, "source_commit": "unknown", "source_state": "unknown"}

    commit = commit_result.stdout.strip()
    if not COMMIT_PATTERN.fullmatch(commit):
        return {"version": version, "source_commit": "unknown", "source_state": "unknown"}
    try:
        status_result = subprocess.run(
            ["git", "-C", str(root), "status", "--porcelain=v1", "--untracked-files=all"],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError):
        state = "unknown"
    else:
        state = "dirty" if status_result.stdout else "clean"
    return {"version": version, "source_commit": commit, "source_state": state}


def validate_tag(tag: str) -> str:
    if not isinstance(tag, str) or not TAG_PATTERN.fullmatch(tag):
        raise ValueError("local image tag must contain 1-128 Docker tag characters")
    return tag


def local_image_references(tag: str) -> dict[str, str]:
    validate_tag(tag)
    return {
        "mcp": f"{LOCAL_NAMESPACE}/arqen-mcp:{tag}",
        "runtime": f"{LOCAL_NAMESPACE}/arqen-runtime:{tag}",
    }


def archive_image_ids(archive_path: Path) -> dict[str, str]:
    try:
        with tarfile.open(archive_path, mode="r:") as image_archive:
            manifest_members = [member for member in image_archive.getmembers() if member.name == "manifest.json"]
            if len(manifest_members) != 1 or not manifest_members[0].isfile():
                raise ValueError("Docker image archive must contain exactly one regular manifest.json")
            manifest_member = manifest_members[0]
            if manifest_member.size > 1_000_000:
                raise ValueError("Docker image archive manifest.json is unexpectedly large")
            manifest_stream = image_archive.extractfile(manifest_member)
            if manifest_stream is None:
                raise ValueError("could not read Docker image archive manifest.json")
            archive_manifest = json.load(manifest_stream)

            if not isinstance(archive_manifest, list):
                raise ValueError("Docker image archive manifest.json must be a list")
            image_ids: dict[str, str] = {}
            for entry in archive_manifest:
                if not isinstance(entry, dict):
                    raise ValueError("Docker image archive manifest contains an invalid entry")
                config_path = entry.get("Config")
                if not isinstance(config_path, str):
                    raise ValueError("Docker image archive manifest entry is missing its image config")
                config_name = PurePosixPath(config_path)
                if config_name.is_absolute() or ".." in config_name.parts:
                    raise ValueError("Docker image archive manifest contains an unsafe config path")
                try:
                    config_member = image_archive.getmember(config_path)
                except KeyError as error:
                    raise ValueError("Docker image archive is missing an image config") from error
                if not config_member.isfile() or config_member.size > 10_000_000:
                    raise ValueError("Docker image archive contains an invalid or oversized image config")
                config_stream = image_archive.extractfile(config_member)
                if config_stream is None:
                    raise ValueError("could not read Docker image archive config")
                config_hash = hashlib.sha256(config_stream.read()).hexdigest()
                image_id = f"sha256:{config_hash}"
                repo_tags = entry.get("RepoTags")
                if not isinstance(repo_tags, list):
                    raise ValueError("Docker image archive manifest entry is missing RepoTags")
                if not all(isinstance(tag, str) for tag in repo_tags):
                    raise ValueError("Docker image archive manifest contains an invalid image tag")
                for tag in repo_tags:
                    previous_image_id = image_ids.get(tag)
                    if previous_image_id is not None and previous_image_id != image_id:
                        raise ValueError(f"Docker image archive tag {tag!r} refers to multiple image configs")
                    image_ids[tag] = image_id
            return image_ids
    except (OSError, tarfile.TarError, json.JSONDecodeError, UnicodeDecodeError) as error:
        raise ValueError(f"could not read Docker image archive: {error}") from error


def archive_sha256(archive_path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with archive_path.open("rb") as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError as error:
        raise ValueError(f"could not read Docker image archive: {error}") from error
    return f"sha256:{digest.hexdigest()}"


def _validate_image_id(image_id: Any, name: str) -> str:
    if not isinstance(image_id, str) or not IMAGE_ID_PATTERN.fullmatch(image_id):
        raise ValueError(f"bundle image {name!r} has an invalid image ID")
    return image_id


def _expected_services(references: dict[str, str]) -> dict[str, str]:
    return {
        "arqen-broker": references["runtime"],
        "arqen-control": references["runtime"],
        "arqen-mcp": references["mcp"],
        **BASE_SERVICES,
    }


def _validate_bundle_data(
    manifest: Any,
    archive_path: Path,
) -> dict[str, Any]:
    if not isinstance(manifest, dict):
        raise ValueError("bundle services.json must contain a JSON object")
    schema_version = manifest.get("schema_version")
    if not isinstance(schema_version, int) or isinstance(schema_version, bool) or schema_version != 1:
        raise ValueError("bundle manifest is legacy or unsupported; regenerate it with make docker-bundle")
    if manifest.get("platform") != "linux/amd64":
        raise ValueError("bundle manifest platform must be linux/amd64")
    version = manifest.get("version")
    if not isinstance(version, str) or not VERSION_PATTERN.fullmatch(version):
        raise ValueError("bundle manifest has an invalid Cargo version")
    source_commit = manifest.get("source_commit")
    if source_commit != "unknown" and (
        not isinstance(source_commit, str) or not COMMIT_PATTERN.fullmatch(source_commit)
    ):
        raise ValueError("bundle manifest has an invalid source commit")
    source_state = manifest.get("source_state")
    if not isinstance(source_state, str) or source_state not in {"clean", "dirty", "unknown"}:
        raise ValueError("bundle manifest has an invalid source state")

    tag = validate_tag(manifest.get("local_image_tag", ""))
    references = local_image_references(tag)
    images = manifest.get("images")
    if not isinstance(images, dict) or set(images) != {"mcp", "runtime"}:
        raise ValueError("bundle manifest is missing image metadata")
    image_ids: dict[str, str] = {}
    for name in ("mcp", "runtime"):
        image = images.get(name)
        if not isinstance(image, dict) or image.get("reference") != references[name]:
            raise ValueError(f"bundle {name} image must use its expected arqen-local reference")
        image_ids[name] = _validate_image_id(image.get("image_id"), name)
    if manifest.get("services") != _expected_services(references):
        raise ValueError("bundle services do not match the local image references and expected base images")
    archive_name = manifest.get("docker_load_archive")
    if not isinstance(archive_name, str) or Path(archive_name).name != archive_name:
        raise ValueError("bundle manifest has an invalid Docker archive filename")

    expected_archive_hash = manifest.get("docker_load_archive_sha256")
    if not isinstance(expected_archive_hash, str) or not re.fullmatch(
        r"^sha256:[0-9a-f]{64}$", expected_archive_hash
    ):
        raise ValueError("bundle manifest has an invalid archive checksum")
    if archive_sha256(archive_path) != expected_archive_hash:
        raise ValueError("bundle archive checksum does not match services.json")
    archive_images = archive_image_ids(archive_path)
    missing_tags = set(references.values()) - set(archive_images)
    if missing_tags:
        raise ValueError("bundle image archive is missing local image references: " + ", ".join(sorted(missing_tags)))
    for name, reference in references.items():
        if archive_images[reference] != image_ids[name]:
            raise ValueError(f"bundle {name} image ID does not match its Docker archive config")

    return {"local_image_tag": tag, "references": references, "image_ids": image_ids}


def validate_bundle(manifest_path: Path, archive_path: Path) -> dict[str, Any]:
    if not manifest_path.is_file():
        raise ValueError(
            f"bundle manifest not found: {manifest_path} (regenerate the bundle with make docker-bundle)"
        )
    if not archive_path.is_file():
        raise ValueError(f"Docker image bundle not found: {archive_path}")
    try:
        with manifest_path.open(encoding="utf-8") as stream:
            manifest = json.load(stream)
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"could not read bundle manifest {manifest_path}: {error}") from error
    return _validate_bundle_data(manifest, archive_path)


def write_bundle_manifest(
    root: Path,
    tag: str,
    archive_path: Path,
    manifest_path: Path,
    mcp_image_id: str,
    runtime_image_id: str,
) -> dict[str, Any]:
    metadata = source_metadata(root)
    references = local_image_references(tag)
    image_ids = {
        "mcp": _validate_image_id(mcp_image_id, "mcp"),
        "runtime": _validate_image_id(runtime_image_id, "runtime"),
    }
    archive_images = archive_image_ids(archive_path)
    missing_tags = set(references.values()) - set(archive_images)
    if missing_tags:
        raise ValueError("Docker image archive is missing local image references: " + ", ".join(sorted(missing_tags)))
    for name, reference in references.items():
        if archive_images[reference] != image_ids[name]:
            raise ValueError(f"bundle {name} image ID does not match its Docker archive config")
    manifest: dict[str, Any] = {
        "schema_version": 1,
        "platform": "linux/amd64",
        "version": metadata["version"],
        "source_commit": metadata["source_commit"],
        "source_state": metadata["source_state"],
        "local_image_tag": tag,
        "images": {
            name: {"reference": references[name], "image_id": image_ids[name]}
            for name in ("mcp", "runtime")
        },
        "services": _expected_services(references),
        "docker_load_archive": archive_path.name,
        "docker_load_archive_sha256": archive_sha256(archive_path),
        "oci_layouts": ["arqen-mcp.oci", "arqen-runtime.oci"],
    }
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    return manifest


def _print_source_metadata(root: Path) -> None:
    metadata = source_metadata(root)
    print("\t".join(metadata[key] for key in ("version", "source_commit", "source_state")))


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    metadata_parser = subparsers.add_parser("source-metadata")
    metadata_parser.add_argument("--root", type=Path, required=True)

    references_parser = subparsers.add_parser("references")
    references_parser.add_argument("--tag", required=True)

    write_parser = subparsers.add_parser("write-bundle-manifest")
    write_parser.add_argument("--root", type=Path, required=True)
    write_parser.add_argument("--tag", required=True)
    write_parser.add_argument("--archive", type=Path, required=True)
    write_parser.add_argument("--manifest", type=Path, required=True)
    write_parser.add_argument("--mcp-image-id", required=True)
    write_parser.add_argument("--runtime-image-id", required=True)

    validate_parser = subparsers.add_parser("validate-bundle")
    validate_parser.add_argument("--archive", type=Path, required=True)
    validate_parser.add_argument("--manifest", type=Path, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    arguments = _parser().parse_args(argv)
    try:
        if arguments.command == "source-metadata":
            _print_source_metadata(arguments.root)
        elif arguments.command == "references":
            references = local_image_references(arguments.tag)
            print(f"{references['mcp']}\t{references['runtime']}")
        elif arguments.command == "write-bundle-manifest":
            write_bundle_manifest(
                arguments.root,
                arguments.tag,
                arguments.archive,
                arguments.manifest,
                arguments.mcp_image_id,
                arguments.runtime_image_id,
            )
        elif arguments.command == "validate-bundle":
            bundle = validate_bundle(arguments.manifest, arguments.archive)
            refs = bundle["references"]
            image_ids = bundle["image_ids"]
            print(
                "\t".join(
                    (
                        bundle["local_image_tag"],
                        refs["mcp"],
                        refs["runtime"],
                        image_ids["mcp"],
                        image_ids["runtime"],
                    )
                )
            )
        return 0
    except (OSError, ValueError, tarfile.TarError, tomllib.TOMLDecodeError) as error:
        print(f"arqen-docker-image-metadata: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
