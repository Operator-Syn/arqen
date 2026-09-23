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
        description = "List bounded Gmail message metadata for the single account selected in Arqen; callers cannot choose an account. Returns target_email, messages (id, thread_id, from, subject, date, labels, snippet, snippet_truncated), next_page_token, and result_size_estimate. Results may be paginated; pass next_page_token as page_token to fetch the next page. Message bodies and attachments are not returned."
    )]
    async fn list_emails(
        &self,
        Parameters(request): Parameters<ListEmailsRequest>,
    ) -> Result<Json<EmailListResponse>, String> {
        let request = request
            .validate()
            .map_err(|error| format!("invalid_request: {error}"))?;
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
