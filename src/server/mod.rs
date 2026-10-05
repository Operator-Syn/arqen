// SPDX-License-Identifier: MPL-2.0
use anyhow::Context;
use arqen::{
    broker::BrokerClient,
    gmail::{
        ActionMarkerRequest, ApplyLabelRequest, ApplyLabelToEmailsRequest, BulkDraftIdsRequest,
        BulkDraftMarkersRequest, BulkEmailMarkersRequest, BulkMessageIdsRequest, BulkResponse,
        CreateDraftRequest, CreateDraftsRequest, CreateLabelRequest, CreateLabelsRequest,
        CreateReplyDraftRequest, CreateReplyDraftsRequest, DeleteLabelRequest, DeleteLabelsRequest,
        DeleteMarkedEmailRequest, DraftActionMark, DraftCreateResult, DraftDeleteResult,
        DraftIdRequest, DraftListResponse, DraftSendResult, EmailDeletionMark, EmailLabel,
        EmailListResponse, EmailReadResponse, EmailReadState, EmailTrashResult, LabelApplyResult,
        LabelDeleteResult, LabelListResponse, ListDraftsRequest, ListEmailsRequest,
        ListLabelsRequest, ReadEmailRequest, ReadEmailsRequest,
    },
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

const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:8787";
const MAX_MCP_REQUEST_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ServerOptions {
    pub listen_addr: SocketAddr,
    pub broker_socket: PathBuf,
    pub allowed_hosts: Vec<String>,
    pub allowed_origins: Vec<String>,
    pub bearer_token: String,
}

mod auth;
mod config;
mod routes;
mod runtime;

use auth::*;
pub use routes::EmailMcpServer;
use routes::*;
pub use runtime::run;
#[cfg(test)]
pub(crate) fn build_router(options: ServerOptions, cancellation: CancellationToken) -> Router {
    runtime::build_router(options, cancellation)
}
#[cfg(test)]
use runtime::request_is_authorized;
#[cfg(test)]
#[path = "../../tests/unit/server/mod.rs"]
mod tests;
