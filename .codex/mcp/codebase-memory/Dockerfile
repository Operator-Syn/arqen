FROM node:22.20.0-bookworm-slim@sha256:b21fe589dfbe5cc39365d0544b9be3f1f33f55f3c86c87a76ff65a02f8f5848e

RUN apt-get update \
    && apt-get install --no-install-recommends --yes ca-certificates git \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /opt/codebase-memory
COPY package.json package-lock.json ./
RUN npm ci --omit=dev \
    && test -x node_modules/codebase-memory-mcp/bin/codebase-memory-mcp \
    && cp node_modules/codebase-memory-mcp/bin/codebase-memory-mcp /usr/local/bin/codebase-memory-mcp \
    && rm -rf node_modules /root/.npm

WORKDIR /workspace/project
ENTRYPOINT ["/usr/local/bin/codebase-memory-mcp"]
