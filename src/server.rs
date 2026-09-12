use anyhow::Context;
use arqen::{
    broker::BrokerClient,
    gmail::{EmailListResponse, ListEmailsRequest},
    mcp::BrokerFailure,
};
use axum::{
    Router,
    extract::{Request, State},
    http::{HeaderValue, StatusCode, header::WWW_AUTHENTICATE},
    middleware::{Next, from_fn_with_state},
    response::{IntoResponse, Response},
    routing::get,
};
use rmcp::{
    Json, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use subtle::ConstantTimeEq;
use tokio_util::sync::CancellationToken;

const DEFAULT_LISTEN_ADDR: &str = "0.0.0.0:8787";
const MAX_MCP_REQUEST_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ServerOptions {
    pub listen_addr: SocketAddr,
    pub broker_socket: PathBuf,
    pub allowed_hosts: Vec<String>,
    pub allowed_origins: Vec<String>,
    pub bearer_token: String,
}

impl ServerOptions {
    pub fn from_env(default_broker_socket: PathBuf) -> anyhow::Result<Self> {
        let listen_addr = std::env::var("ARQEN_MCP_LISTEN_ADDR")
            .unwrap_or_else(|_| DEFAULT_LISTEN_ADDR.into())
            .parse()
            .context("parse ARQEN_MCP_LISTEN_ADDR")?;
        let broker_socket = std::env::var_os("ARQEN_GMAIL_BROKER_SOCKET")
            .map(PathBuf::from)
            .unwrap_or(default_broker_socket);
        let allowed_hosts = split_list_env("ARQEN_MCP_ALLOWED_HOSTS")?;
        let allowed_origins = split_list_env("ARQEN_MCP_ALLOWED_ORIGINS")?;
        anyhow::ensure!(
            !allowed_hosts.is_empty(),
            "ARQEN_MCP_ALLOWED_HOSTS must contain at least one public Host value"
        );
        anyhow::ensure!(
            !allowed_origins.is_empty(),
            "ARQEN_MCP_ALLOWED_ORIGINS must contain at least one allowed Origin"
        );
        let bearer_token = bearer_token_from_env()?;
        validate_secret(&bearer_token, "MCP bearer token")?;
        Ok(Self {
            listen_addr,
            broker_socket,
            allowed_hosts,
            allowed_origins,
            bearer_token,
        })
    }
}

fn split_list_env(name: &str) -> anyhow::Result<Vec<String>> {
    let raw = std::env::var(name).with_context(|| format!("set {name}"))?;
    split_values(name, &raw)
}

fn split_values(name: &str, raw: &str) -> anyhow::Result<Vec<String>> {
    let values: Vec<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect();
    anyhow::ensure!(
        !values
            .iter()
            .any(|value| value.chars().any(char::is_control)),
        "{name} contains control characters"
    );
    Ok(values)
}

fn bearer_token_from_env() -> anyhow::Result<String> {
    match std::env::var("ARQEN_MCP_BEARER_TOKEN") {
        Ok(token) => Ok(token),
        Err(std::env::VarError::NotPresent) => {
            let path = std::env::var_os("ARQEN_MCP_BEARER_TOKEN_FILE")
                .context("set ARQEN_MCP_BEARER_TOKEN or ARQEN_MCP_BEARER_TOKEN_FILE")?;
            let content = std::fs::read_to_string(&path).with_context(|| {
                format!(
                    "read MCP bearer token file at {}",
                    PathBuf::from(&path).display()
                )
            })?;
            Ok(content
                .strip_suffix("\r\n")
                .or_else(|| content.strip_suffix('\n'))
                .unwrap_or(&content)
                .to_owned())
        }
        Err(std::env::VarError::NotUnicode(_)) => {
            anyhow::bail!("ARQEN_MCP_BEARER_TOKEN is not valid UTF-8")
        }
    }
}

fn validate_secret(value: &str, name: &str) -> anyhow::Result<()> {
    anyhow::ensure!(!value.is_empty(), "{name} cannot be empty");
    anyhow::ensure!(
        !value.chars().any(char::is_control),
        "{name} contains unsupported control characters"
    );
    Ok(())
}

#[derive(Clone)]
struct AuthState {
    bearer_token: Arc<String>,
}

#[derive(Clone)]
pub struct EmailMcpServer {
    broker: BrokerClient,
    tool_router: ToolRouter<Self>,
}

impl EmailMcpServer {
    fn new(broker: BrokerClient) -> Self {
        Self {
            broker,
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl EmailMcpServer {
    #[tool(
        name = "list_emails",
        description = "List bounded metadata summaries from the explicitly selected Gmail account."
    )]
    async fn list_emails(
        &self,
        Parameters(request): Parameters<ListEmailsRequest>,
    ) -> Result<Json<EmailListResponse>, String> {
        self.broker
            .list_emails(request)
            .await
            .map(Json)
            .map_err(format_broker_failure)
    }
}

fn format_broker_failure(error: BrokerFailure) -> String {
    format!("{}: {}", error.code.as_str(), error.message)
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for EmailMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "This server exposes read-only Gmail summaries for one account selected in Arqen.",
        )
    }
}

pub fn run(options: ServerOptions) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("create MCP server runtime")?;
    runtime.block_on(run_async(options))
}

async fn run_async(options: ServerOptions) -> anyhow::Result<()> {
    let cancellation_token = CancellationToken::new();
    let listen_addr = options.listen_addr;
    let router = build_router(options, cancellation_token.clone());
    let listener = tokio::net::TcpListener::bind(listen_addr)
        .await
        .with_context(|| format!("bind MCP server at {listen_addr}"))?;
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal(cancellation_token))
        .await
        .context("serve Streamable HTTP MCP endpoint")
}

fn build_router(options: ServerOptions, cancellation_token: CancellationToken) -> Router {
    let config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .with_max_request_body_bytes(MAX_MCP_REQUEST_BYTES)
        .with_allowed_hosts(options.allowed_hosts)
        .with_allowed_origins(options.allowed_origins)
        .with_cancellation_token(cancellation_token);
    let broker = BrokerClient::new(options.broker_socket);
    let service = StreamableHttpService::new(
        move || Ok(EmailMcpServer::new(broker.clone())),
        LocalSessionManager::default().into(),
        config,
    );
    let auth_state = AuthState {
        bearer_token: Arc::new(options.bearer_token),
    };
    Router::new()
        .nest_service("/mcp", service)
        .route("/healthz", get(health))
        .layer(from_fn_with_state(auth_state, authorize))
}

async fn health() -> impl IntoResponse {
    (StatusCode::NO_CONTENT, ())
}

async fn authorize(State(state): State<AuthState>, request: Request, next: Next) -> Response {
    if request_is_authorized(&request, &state.bearer_token) {
        return next.run(request).await;
    }
    let mut response = StatusCode::UNAUTHORIZED.into_response();
    response.headers_mut().insert(
        WWW_AUTHENTICATE,
        HeaderValue::from_static("Bearer realm=\"arqen-mcp\""),
    );
    response
}

fn request_is_authorized<B>(request: &axum::http::Request<B>, expected: &str) -> bool {
    let Some(value) = request.headers().get("authorization") else {
        return false;
    };
    let Ok(value) = value.to_str() else {
        return false;
    };
    let Some(provided) = value.strip_prefix("Bearer ") else {
        return false;
    };
    !provided.is_empty() && provided.as_bytes().ct_eq(expected.as_bytes()).into()
}

async fn shutdown_signal(cancellation_token: CancellationToken) {
    let _ = tokio::signal::ctrl_c().await;
    cancellation_token.cancel();
}

#[cfg(test)]
mod tests {
    use super::{
        ServerOptions, build_router, request_is_authorized, split_values, validate_secret,
    };
    use arqen::gmail::EmailListResponse;
    #[cfg(unix)]
    use arqen::mcp::{BrokerRequest, BrokerResponse};
    use axum::http::Request;
    use reqwest::StatusCode;
    use std::{net::SocketAddr, path::PathBuf};
    use tokio_util::sync::CancellationToken;

    fn test_options(address: SocketAddr) -> ServerOptions {
        ServerOptions {
            listen_addr: address,
            broker_socket: PathBuf::from("/tmp/arqen-test-missing-broker.sock"),
            allowed_hosts: vec![address.to_string()],
            allowed_origins: vec![format!("http://{address}")],
            bearer_token: "test-secret".into(),
        }
    }

    #[test]
    fn bearer_authorization_requires_exact_scheme_and_value() {
        let request = Request::builder()
            .header("authorization", "Bearer secret")
            .body(())
            .unwrap();
        assert!(request_is_authorized(&request, "secret"));
        let wrong = Request::builder()
            .header("authorization", "Bearer other")
            .body(())
            .unwrap();
        assert!(!request_is_authorized(&wrong, "secret"));
        let basic = Request::builder()
            .header("authorization", "Basic secret")
            .body(())
            .unwrap();
        assert!(!request_is_authorized(&basic, "secret"));
    }

    #[test]
    fn bearer_secret_validation_rejects_empty_and_control_values() {
        assert!(validate_secret("", "token").is_err());
        assert!(validate_secret("secret\n", "token").is_err());
        assert!(validate_secret("secret", "token").is_ok());
    }

    #[test]
    fn split_list_values_require_an_explicit_environment_value() {
        assert_eq!(
            split_values("ARQEN_TEST_LIST", "example.com, https://example.com").unwrap(),
            vec!["example.com", "https://example.com"]
        );
    }

    #[tokio::test]
    async fn http_surface_requires_bearer_auth_and_exposes_tools() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let cancellation = CancellationToken::new();
        let router = build_router(test_options(address), cancellation.clone());
        let shutdown = cancellation.clone();
        tokio::spawn(async move {
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(shutdown.cancelled_owned())
                .await;
        });
        let client = reqwest::Client::new();
        let base = format!("http://{address}");

        let unauthorized = client.get(format!("{base}/healthz")).send().await.unwrap();
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            unauthorized
                .headers()
                .get("www-authenticate")
                .and_then(|value| value.to_str().ok()),
            Some("Bearer realm=\"arqen-mcp\"")
        );

        let health = client
            .get(format!("{base}/healthz"))
            .bearer_auth("test-secret")
            .send()
            .await
            .unwrap();
        assert_eq!(health.status(), StatusCode::NO_CONTENT);

        let initialize = client
            .post(format!("{base}/mcp"))
            .bearer_auth("test-secret")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .body(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}"#)
            .send()
            .await
            .unwrap();
        assert_eq!(initialize.status(), StatusCode::OK);
        let body: serde_json::Value = initialize.json().await.unwrap();
        assert!(body["result"]["capabilities"]["tools"].is_object());

        let tools = client
            .post(format!("{base}/mcp"))
            .bearer_auth("test-secret")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("origin", format!("http://{address}"))
            .body(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#)
            .send()
            .await
            .unwrap();
        assert_eq!(tools.status(), StatusCode::OK);
        let body: serde_json::Value = tools.json().await.unwrap();
        assert_eq!(body["result"]["tools"][0]["name"], "list_emails");

        let rejected_host = client
            .post(format!("{base}/mcp"))
            .bearer_auth("test-secret")
            .header("host", "unexpected.example")
            .header("content-type", "application/json")
            .body(r#"{"jsonrpc":"2.0","id":4,"method":"tools/list","params":{}}"#)
            .send()
            .await
            .unwrap();
        assert_eq!(rejected_host.status(), StatusCode::FORBIDDEN);

        let rejected_origin = client
            .post(format!("{base}/mcp"))
            .bearer_auth("test-secret")
            .header("origin", "https://unexpected.example")
            .header("content-type", "application/json")
            .body(r#"{"jsonrpc":"2.0","id":5,"method":"tools/list","params":{}}"#)
            .send()
            .await
            .unwrap();
        assert_eq!(rejected_origin.status(), StatusCode::FORBIDDEN);

        cancellation.cancel();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn tool_call_forwards_only_the_bounded_list_request_to_the_broker() {
        use std::{
            io::{BufRead, BufReader, Write},
            os::unix::net::UnixListener,
            thread,
        };

        let socket_path = std::env::temp_dir().join(format!(
            "arqen-server-test-{}.sock",
            uuid::Uuid::new_v4().simple()
        ));
        let listener = UnixListener::bind(&socket_path).unwrap();
        let broker_thread = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let request: BrokerRequest = serde_json::from_str(&line).unwrap();
            let request = request.validate().unwrap();
            assert_eq!(request.query.as_deref(), Some("from:sender@example.com"));
            assert_eq!(request.max_results, 3);
            let response = BrokerResponse::Ok {
                result: EmailListResponse {
                    target_email: "target@example.com".into(),
                    messages: Vec::new(),
                    next_page_token: None,
                    result_size_estimate: Some(0),
                },
            };
            let mut stream = reader.into_inner();
            serde_json::to_writer(&mut stream, &response).unwrap();
            stream.write_all(b"\n").unwrap();
        });

        let tcp_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = tcp_listener.local_addr().unwrap();
        let cancellation = CancellationToken::new();
        let mut options = test_options(address);
        options.broker_socket = socket_path.clone();
        let router = build_router(options, cancellation.clone());
        let shutdown = cancellation.clone();
        tokio::spawn(async move {
            let _ = axum::serve(tcp_listener, router)
                .with_graceful_shutdown(shutdown.cancelled_owned())
                .await;
        });
        let response = reqwest::Client::new()
            .post(format!("http://{address}/mcp"))
            .bearer_auth("test-secret")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .body(r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_emails","arguments":{"query":" from:sender@example.com ","max_results":3}}}"#)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(
            body["result"]["structuredContent"]["target_email"],
            "target@example.com"
        );
        broker_thread.join().unwrap();
        cancellation.cancel();
        let _ = std::fs::remove_file(socket_path);
    }
}
