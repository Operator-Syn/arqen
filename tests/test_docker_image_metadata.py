# SPDX-License-Identifier: MPL-2.0
import importlib.util
import hashlib
import io
import json
import subprocess
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest import mock


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "arqen-docker-image-metadata.py"
SPEC = importlib.util.spec_from_file_location("arqen_docker_image_metadata", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class DockerImageMetadataTests(unittest.TestCase):
    def make_project(self) -> Path:
        temporary_directory = tempfile.TemporaryDirectory()
        self.addCleanup(temporary_directory.cleanup)
        root = Path(temporary_directory.name)
        (root / "Cargo.toml").write_text(
            '[package]\nname = "arqen"\nversion = "0.4.2"\n',
            encoding="utf-8",
        )
        return root

    def make_archive(self, root: Path, tags: list[str]) -> Path:
        archive = root / "docker-images-linux-amd64.tar"
        entries = []
        configs = {}
        for tag in tags:
            config = json.dumps({"test_image": tag}, sort_keys=True).encode()
            config_name = hashlib.sha256(config).hexdigest() + ".json"
            configs[config_name] = config
            entries.append({"Config": config_name, "RepoTags": [tag], "Layers": []})
        payload = json.dumps(entries).encode()
        with tarfile.open(archive, mode="w") as image_archive:
            manifest_info = tarfile.TarInfo("manifest.json")
            manifest_info.size = len(payload)
            image_archive.addfile(manifest_info, io.BytesIO(payload))
            for config_name, config in configs.items():
                config_info = tarfile.TarInfo(config_name)
                config_info.size = len(config)
                image_archive.addfile(config_info, io.BytesIO(config))
        return archive

    def make_valid_bundle(self, root: Path) -> tuple[Path, Path]:
        tag = "dev-worktree"
        references = MODULE.local_image_references(tag)
        archive = self.make_archive(root, list(references.values()))
        image_ids = MODULE.archive_image_ids(archive)
        manifest_path = root / "services.json"
        MODULE.write_bundle_manifest(
            root,
            tag,
            archive,
            manifest_path,
            image_ids[references["mcp"]],
            image_ids[references["runtime"]],
        )
        return archive, manifest_path

    def test_local_references_use_one_validated_moving_tag(self) -> None:
        self.assertEqual(
            MODULE.local_image_references("dev-worktree"),
            {
                "mcp": "arqen-local/arqen-mcp:dev-worktree",
                "runtime": "arqen-local/arqen-runtime:dev-worktree",
            },
        )
        for invalid_tag in ("", "tag:bad", "bad/tag", "white space"):
            with self.subTest(tag=invalid_tag), self.assertRaises(ValueError):
                MODULE.local_image_references(invalid_tag)

    def test_source_metadata_records_clean_dirty_and_missing_git_states(self) -> None:
        root = self.make_project()
        commit = "c" * 40
        with mock.patch.object(
            MODULE.subprocess,
            "run",
            side_effect=[
                subprocess.CompletedProcess([], 0, commit + "\n", ""),
                subprocess.CompletedProcess([], 0, "", ""),
            ],
        ):
            self.assertEqual(
                MODULE.source_metadata(root),
                {"version": "0.4.2", "source_commit": commit, "source_state": "clean"},
            )
        with mock.patch.object(
            MODULE.subprocess,
            "run",
            side_effect=[
                subprocess.CompletedProcess([], 0, commit + "\n", ""),
                subprocess.CompletedProcess([], 0, " M src/main.rs\n", ""),
            ],
        ):
            self.assertEqual(MODULE.source_metadata(root)["source_state"], "dirty")
        with mock.patch.object(MODULE.subprocess, "run", side_effect=FileNotFoundError):
            self.assertEqual(
                MODULE.source_metadata(root),
                {"version": "0.4.2", "source_commit": "unknown", "source_state": "unknown"},
            )

    def test_bundle_manifest_round_trips_with_archive_and_image_ids(self) -> None:
        root = self.make_project()
        archive, manifest_path = self.make_valid_bundle(root)
        metadata = MODULE.validate_bundle(manifest_path, archive)
        self.assertEqual(metadata["local_image_tag"], "dev-worktree")
        self.assertEqual(
            metadata["references"],
            {
                "mcp": "arqen-local/arqen-mcp:dev-worktree",
                "runtime": "arqen-local/arqen-runtime:dev-worktree",
            },
        )
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        self.assertEqual(manifest["version"], "0.4.2")
        self.assertEqual(manifest["source_state"], "unknown")
        self.assertEqual(manifest["services"]["arqen-control"], manifest["services"]["arqen-broker"])

    def test_missing_malformed_and_legacy_manifests_fail_closed(self) -> None:
        root = self.make_project()
        archive = self.make_archive(root, ["arqen-local/arqen-mcp:dev", "arqen-local/arqen-runtime:dev"])
        missing = root / "missing.json"
        with self.assertRaisesRegex(ValueError, "manifest not found"):
            MODULE.validate_bundle(missing, archive)
        _, valid_manifest = self.make_valid_bundle(root)
        with self.assertRaisesRegex(ValueError, "Docker image bundle not found"):
            MODULE.validate_bundle(valid_manifest, root / "not-found.tar")

        malformed = root / "malformed.json"
        malformed.write_text("{not json", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "could not read bundle manifest"):
            MODULE.validate_bundle(malformed, archive)

        legacy = root / "legacy.json"
        legacy.write_text(
            json.dumps(
                {
                    "platform": "linux/amd64",
                    "version": "0.4.2",
                    "services": {
                        "arqen-mcp": "ghcr.io/operator-syn/arqen-mcp:0.4.2",
                        "arqen-control": "ghcr.io/operator-syn/arqen-runtime:0.4.2",
                        "arqen-broker": "ghcr.io/operator-syn/arqen-runtime:0.4.2",
                    },
                }
            ),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(ValueError, "legacy or unsupported"):
            MODULE.validate_bundle(legacy, archive)

    def test_archive_checksum_reference_and_manifest_mismatches_are_rejected(self) -> None:
        root = self.make_project()
        archive, manifest_path = self.make_valid_bundle(root)
        archive.write_bytes(archive.read_bytes() + b"changed")
        with self.assertRaisesRegex(ValueError, "checksum does not match"):
            MODULE.validate_bundle(manifest_path, archive)

        refs = MODULE.local_image_references("dev")
        missing_image_archive = self.make_archive(root, [refs["mcp"]])
        with self.assertRaisesRegex(ValueError, "missing local image references"):
            MODULE.write_bundle_manifest(
                root,
                "dev",
                missing_image_archive,
                root / "missing-image.json",
                "sha256:" + "a" * 64,
                "sha256:" + "b" * 64,
            )

        archive, manifest_path = self.make_valid_bundle(root)
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        manifest["images"]["mcp"]["reference"] = "ghcr.io/operator-syn/arqen-mcp:0.4.2"
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "expected arqen-local reference"):
            MODULE.validate_bundle(manifest_path, archive)

        archive, manifest_path = self.make_valid_bundle(root)
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        manifest["images"]["mcp"]["image_id"] = "sha256:" + "c" * 64
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "image ID does not match"):
            MODULE.validate_bundle(manifest_path, archive)


if __name__ == "__main__":
    unittest.main()
