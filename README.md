# Arqen

**Use your Gmail account with an AI assistant, while you stay in control of
which account it can access.**

Arqen is an app you run on your own computer. Sign in to Google, choose one
account, and connect an MCP-compatible assistant to Arqen. MCP is a way for an
assistant to call tools provided by another app.

## Try Arqen

Follow the [Docker Quickstart](docs/quickstart.md). It pulls the published
Arqen images; you do not need to build the app or install a Rust toolchain.

You will need Docker Compose and your own Google OAuth Desktop client JSON.
Arqen does not include Google credentials. The Quickstart explains how to
create and add yours.

## What Arqen can do

| You choose and manage | Your assistant can |
| --- | --- |
| Connect Google accounts and choose which one to share | List and read email in the chosen account |
| Reconnect or remove an account | List, create, and remove Gmail labels |
| Keep Google sign-in data protected on this computer | Mark a message read or unread |

Your assistant cannot choose another account or receive your Google password
or refresh token. Arqen asks Google for Gmail read and modify permissions. The
modify permission is broader than the current tools: it can allow composing
and sending email, which Arqen does not offer. Read the [data and permissions
guide](docs/security/auth-and-data.md) before authorizing access.

The Docker Quickstart keeps the control page and MCP endpoint on your computer.
Access from other devices needs a TLS reverse proxy; see [container and proxy
setup](docs/operations/container-and-nginx.md).

## For developers

Make-based commands support source builds, local services, and project checks.
Start with the [development workflows](docs/development/local-workflows.md),
or enter the Rust environment and run the checks:

```bash
nix develop .#arqen
make check
```

## Documentation

| If you want to… | Read… |
| --- | --- |
| Install and try Arqen | [Docker Quickstart](docs/quickstart.md) |
| Understand account and credential handling | [Data and permissions](docs/security/auth-and-data.md) |
| Connect an MCP client or inspect its tools | [MCP API](docs/api/mcp.md) |
| Configure a reverse proxy | [Container and proxy setup](docs/operations/container-and-nginx.md) |
| Build or run from source | [Development workflows](docs/development/local-workflows.md) |
| Find or add tests | [Test layout](docs/development/testing.md) |
| Browse all guides | [Documentation map](docs/README.md) |

## License

Arqen is distributed under the **Mozilla Public License 2.0 (MPL-2.0)**. See
[`LICENSE`](LICENSE) for the complete terms.
