# Test layout

Keep test code under the repository's root `tests/` directory:

- `tests/unit/` contains Rust unit modules grouped by their owning `src/`
  module. Source modules attach them with `#[cfg(test)]` and `#[path]`, which
  keeps tests able to inspect private implementation details.
- `tests/python/` contains Python `unittest` suites. Run them with
  `python3 -m unittest discover -s tests/python`.
- `tests/integration/` is reserved for black-box suites organized by language
  or system boundary. Declare Cargo integration targets with paths under this
  directory so their source stays categorized here.

Add tests under the closest matching subtree. Keep test source out of `src/`
and avoid loose test files at the root of `tests/`. `cargo test` remains the
Rust test entry point and discovers the centralized unit modules through their
source owners.
