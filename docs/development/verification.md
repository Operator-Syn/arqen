# Development and verification

Use the pinned Nix environment when available:

```bash
nix develop .#arqen -c cargo fmt --check
nix develop .#arqen -c shellcheck -e SC1091 deploy/containers/openbao/bootstrap.sh scripts/arqen-docker-up.sh scripts/arqen-openbao-smoke.sh
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
same authenticated protocol checks with a disposable legacy Docker Compose
service. `make docker-setup` and `make docker-up` exercise the persistent
Docker-native profile; its clean-slate smoke includes OpenBao initialization,
unseal recovery, streamed TUI HTTP access, broker/MCP readiness, and role
policy checks. These protocol paths do not call Google. Add `--call`
(`make smoke-local-call` or `make compose-smoke-call`) only for an intentional
live `list_emails` request using the selected account and the configured
protected credential store.

`make openbao-smoke` starts a disposable OpenBao development server with
test-only credentials. It checks initial AppRole provisioning, idempotent
startup, independent repair of stale control and broker credentials, unchanged
KV data, and no credential rotation when an AppRole login cannot be completed.
It does not access the configured Docker-native OpenBao volume, OAuth client
configuration, keyring, Google account, or live refresh tokens.

The unit tests cover SQLite target invariants, TUI target rendering and
keyboard behavior, callback routes, the control gateway's password/session
boundary and HTTP/WebSocket proxy, bounded broker frames, Gmail request
construction/error mapping, bearer authentication, Host/Origin checks, MCP
tool discovery, readiness failures, and configured remote callback ports. Local
HTTP tests use loopback fixtures; they do not call Google or require a keyring
secret.

The service templates and quickstart are source-level deployment artifacts.
`systemctl --user` activation, user lingering, headless Secret Service
availability, and the SSH-tunneled real OAuth flow require operator-owned
verification on the target VPS.

Keep evidence categories separate:

| Evidence | Establishes | Does not establish |
| --- | --- | --- |
| Format/tests/lint/build | Source compiles and local contracts pass | Live OAuth, Gmail access, TLS, or deployment |
| `nix flake check --no-build` | Flake evaluates without building | Activation or runtime service health |
| Container/proxy review | Templates are structurally reviewable | A deployed public endpoint |
| Manual authenticated run | Operator’s selected account works at that time | Ongoing consent, policy approval, or multi-user safety |
