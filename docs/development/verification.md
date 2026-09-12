# Development and verification

Use the pinned Nix environment when available:

```bash
nix develop .#arqen -c cargo fmt --check
nix develop .#arqen -c cargo test
nix develop .#arqen -c cargo clippy --all-targets --all-features -- -D warnings
nix develop .#arqen -c cargo build
nix flake check --no-build
git diff --check
```

The same sequence is available through the named workflow:

```bash
make setup-local   # once; copies .env.example and generates an ignored token
make check
```

`make check` (or `scripts/arqen-check.sh`) runs the Cargo checks through the
pinned `.#arqen` shell when Nix is installed and otherwise uses the system
toolchain. It also runs `nix flake check --no-build` when `nix` is available.
The wrappers source `.env`; they never print bearer tokens or OAuth JSON.

`make smoke-local` exercises the native broker and MCP processes on disposable
loopback ports and a temporary Unix socket. `make compose-smoke` performs the
same authenticated protocol checks with a disposable Docker Compose service.
Both stop their processes/containers and remove only the temporary paths they
created. Neither test calls Google. Add `--call` (`make smoke-local-call` or
`make compose-smoke-call`) only for an intentional live `list_emails` request
using the selected account and host keyring.

The unit tests cover SQLite target invariants, TUI target rendering and
keyboard behavior, callback routes, bounded broker frames, Gmail request
construction/error mapping, bearer authentication, Host/Origin checks, and
MCP tool discovery. Local HTTP tests use loopback fixtures; they do not call
Google or require a keyring secret.

Keep evidence categories separate:

| Evidence | Establishes | Does not establish |
| --- | --- | --- |
| Format/tests/lint/build | Source compiles and local contracts pass | Live OAuth, Gmail access, TLS, or deployment |
| `nix flake check --no-build` | Flake evaluates without building | Activation or runtime service health |
| Container/proxy review | Templates are structurally reviewable | A deployed public endpoint |
| Manual authenticated run | Operator’s selected account works at that time | Ongoing consent, policy approval, or multi-user safety |
