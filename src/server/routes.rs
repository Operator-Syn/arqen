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
