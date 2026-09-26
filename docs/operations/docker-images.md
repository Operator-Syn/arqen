# Docker image releases

The `Publish Docker images` workflow is triggered by pushes to `main` that
change an image input. Documentation-only and other unrelated changes do not
publish new images. It validates the root Cargo package's stable `X.Y.Z`
version, builds and publishes the MCP, runtime, and project OpenBao images for
`linux/amd64` and `linux/arm64`, then writes a source tag and bumps only the
patch version in `Cargo.toml` and `Cargo.lock`.

The two architectures build in parallel on native GitHub-hosted runners. Each
architecture job builds all three images and pushes commit-scoped staging tags;
the publish job combines them into versioned and `latest` multi-platform
indexes only after both architecture jobs succeed. BuildKit uses separate
GitHub Actions cache scopes for each image and architecture. A same-commit
rerun can reuse completed build layers, while source changes still rebuild the
affected Rust layer.

| Image | Services | Tags |
| --- | --- | --- |
| `ghcr.io/operator-syn/arqen-mcp` | `arqen-mcp` | `X.Y.Z`, `latest` |
| `ghcr.io/operator-syn/arqen-runtime` | `arqen-control`, `arqen-broker` | `X.Y.Z`, `latest` |
| `ghcr.io/operator-syn/arqen-openbao` | OpenBao with Arqen policies and automatic first-run bootstrap | `X.Y.Z`, `latest` |

Each published source version is also tagged `vX.Y.Z`. After a normal
successful publication, the workflow advances only the patch version.
Workflow concurrency serializes active releases; GitHub may replace an older
pending run when newer pushes arrive.
The source tag and current main version checks make a rerun safe after partial
completion. A manually selected minor or major version in Cargo files is used
for the next successful publication and then only its patch is incremented.
Before starting architecture builds, the workflow checks whether the current
version tag is already used. If it points to an ancestor in the same `main`
history, the workflow automatically advances the patch version, commits the
manifest and lockfile update, and builds from that new source commit. That
version remains available for retry if a later build or publish step fails. A
tag from unrelated history still stops the run before build compute is spent.
When setting a major or minor release, edit `Cargo.toml` and run `cargo check`
to update the root package version in `Cargo.lock` before pushing.
The workflow requires GitHub Actions `contents: write` and `packages: write`
permissions, and repository branch rules must allow its direct patch-bump
commit to `main`. It uses `GITHUB_TOKEN`; those commits do not start another
push workflow.

## On-demand branch beta images

Pushes to `main` continue to publish stable `X.Y.Z` and `latest` tags. To publish
a beta from another branch, open **Actions → Publish Docker images → Run
workflow**, choose the branch, and start the run. Manual dispatch on `main` is
rejected before image builds; tag dispatches are rejected, and non-`main` pushes
do not publish automatically.
The workflow must first be present on the repository's default branch for the
manual run button to be available ([GitHub instructions](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/manually-run-a-workflow)).

The beta image version is `X.Y.Z-beta.<branch-slug>.<run-number>`, based on the
selected branch's Cargo version. Branch names are lowercased and normalized for
SemVer and Docker tags. For example, branch `feature/openbao-setup` might
publish `0.1.2-beta.feature-openbao-setup.42`. Beta runs publish that tag for
all three images and write digest metadata to
`prereleases/<version>.json` on `docker-images`. They do not update stable
`latest.json`, stable tags, source tags, or Cargo versions.

To use a beta MCP/runtime release in the Docker-native stack, set the exact
published tag in the ignored `.env` file and run `make docker-up`:

```dotenv
ARQEN_DOCKER_IMAGE_SOURCE=registry
ARQEN_DOCKER_IMAGE_TAG=0.1.2-beta.feature-openbao-setup.42
```

Registry mode pulls that versioned multi-platform tag for MCP and runtime.
The Docker-native stack continues to use upstream `openbao/openbao:2.6.0`; the
project OpenBao beta remains available as a GHCR package but is not consumed by
this Compose profile.

The `docker-images` branch is an orphan metadata branch. It contains
`releases/<version>.json`, `prereleases/<version>.json`, `latest.json`, and a
short index README, including image references, manifest digests, supported
platforms, source commit, and pull commands. Versioned tags are accompanied by
content digests. Image layers, source archives, and binaries are not stored on
that branch.

The recorded image digests identify the published multi-platform indexes. The
metadata branch initialization handles an empty orphan index before adding
only the release metadata files.

## GHCR visibility

GitHub creates packages private by default. After the workflow's first
successful publication, open all three packages' GitHub settings and change
their visibility to public. Until then, anonymous pulls will fail. The workflow
does not change package visibility or weaken repository branch protection.

## Local use

Source builds remain the default:

```bash
make docker-up
```

Local source builds use `arqen-local/arqen-mcp:dev` and
`arqen-local/arqen-runtime:dev`; they never overwrite GHCR references. The
Cargo version, Git revision, and clean/dirty/unknown worktree state are recorded
as image labels. `ARQEN_DOCKER_LOCAL_IMAGE_TAG` can select another local tag
when multiple checkouts share a Docker daemon. Local and registry images remain
cached independently; switching modes does not prune either cache.

To pull from GHCR before startup, set the image source and optional release tag
in the ignored `.env` file. `ARQEN_DOCKER_IMAGE_TAG` applies only to registry
mode; a stale value there does not affect local builds or bundles:

```dotenv
ARQEN_DOCKER_IMAGE_SOURCE=registry
ARQEN_DOCKER_IMAGE_TAG=0.1.0
```

For a local AMD64 transfer bundle:

```bash
make docker-bundle
```

This writes `out/arqen-docker-stack/` with the `arqen-mcp.oci` and
`arqen-runtime.oci` OCI layouts, a Docker-loadable archive containing app and
Compose base images, and `services.json` mapping Compose services to image
references. App images use local-only references and the tag selected by
`ARQEN_DOCKER_LOCAL_IMAGE_TAG`; the manifest records Cargo version, source
revision/state, image IDs, and the archive checksum. Bundle startup validates
the manifest and archive before loading images or starting Compose. Missing,
modified, or legacy GHCR-tagged bundles fail with instructions to regenerate
them. Set `ARQEN_DOCKER_IMAGE_BUNDLE_MANIFEST` when the manifest is stored
outside the archive's directory. It uses a temporary OCI-capable Buildx builder
because the default Docker builder cannot export OCI layouts. Configure startup
with:

```dotenv
ARQEN_DOCKER_IMAGE_SOURCE=bundle
```

The bundle is ignored by Git and Docker build contexts. It never includes
`.secrets`, OAuth client JSON, refresh tokens, OpenBao credentials, bearer
tokens, or local account databases. Compose continues to mount those runtime
files through its existing secret boundary.

After a successful GHCR release, the workflow advances `Cargo.toml` to the next
patch version. A local image can therefore report a version newer than GHCR's
current stable release; its local reference and source labels distinguish it
from a published image. The Docker-native Compose stack continues to use
upstream `openbao/openbao:2.6.0`; the project OpenBao image remains a
CI-published image.
