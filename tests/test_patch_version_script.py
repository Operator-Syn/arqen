# SPDX-License-Identifier: MPL-2.0
import importlib.util
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "arqen-bump-patch-version.py"
SPEC = importlib.util.spec_from_file_location("arqen_bump_patch_version", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class PatchVersionBumpTests(unittest.TestCase):
    def make_project(self, version: str) -> Path:
        temporary_directory = tempfile.TemporaryDirectory()
        self.addCleanup(temporary_directory.cleanup)
        root = Path(temporary_directory.name)
        (root / "Cargo.toml").write_text(
            f'[package]\nname = "arqen"\nversion = "{version}"\n\n'
            '[dependencies]\nexample = { version = "1.2.3" }\n',
            encoding="utf-8",
        )
        (root / "Cargo.lock").write_text(
            '[[package]]\nname = "dependency"\nversion = "9.8.7"\n\n'
            '[[package]]\nname = "arqen"\nversion = "' + version + '"\n',
            encoding="utf-8",
        )
        return root

    def test_bumps_patch_and_only_root_package_versions(self) -> None:
        root = self.make_project("0.4.9")
        self.assertEqual(MODULE.bump(root), "0.4.10")
        self.assertIn('version = "0.4.10"', (root / "Cargo.toml").read_text())
        self.assertIn('version = "1.2.3"', (root / "Cargo.toml").read_text())
        self.assertIn('name = "dependency"\nversion = "9.8.7"', (root / "Cargo.lock").read_text())
        self.assertIn('name = "arqen"\nversion = "0.4.10"', (root / "Cargo.lock").read_text())

    def test_rejects_non_numeric_release_versions(self) -> None:
        with self.assertRaises(ValueError):
            MODULE.bump(self.make_project("1.2.3-rc.1"))


if __name__ == "__main__":
    unittest.main()
