# Arqen image release index

This branch contains release metadata only. Image layers are published
to GHCR and are not stored here.

- `latest.json` identifies the most recently published stable image release.
- `releases/<version>.json` records immutable references, digests,
  source commit, platforms, and pull commands for each release.
- `prereleases/<version>.json` records manually published branch beta
  images without changing the stable `latest.json` index.
- `ghcr.io/operator-syn/arqen-runtime` is shared by the control and
  broker services; `ghcr.io/operator-syn/arqen-mcp` serves MCP; and
  `ghcr.io/operator-syn/arqen-openbao` bootstraps project OpenBao.
