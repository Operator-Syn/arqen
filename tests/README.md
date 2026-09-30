# Test tree

All repository test files live under this directory.

- `unit/` contains Rust unit-test modules, grouped to mirror the owning `src/`
  module. Source modules include these files with `#[cfg(test)]` and `#[path]`
  so unit tests retain access to private implementation details.
- `python/` contains Python `unittest` suites. Run them with
  `python3 -m unittest discover -s tests`.
- `integration/` is reserved for black-box tests that cross process or service
  boundaries. Keep suites in language- or boundary-specific subdirectories.

Add new tests under the closest matching subtree; do not put test files back
under `src/` or loose in this directory. `cargo test` remains the Rust test
entry point and includes the centralized unit modules through their source
owners.
