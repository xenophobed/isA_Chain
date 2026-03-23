use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info};

use crate::blockchain::Blockchain;
use crate::types::ChainId;
use super::handlers::RpcHandler;
use super::types::*;

#[derive(Clone)]
pub struct AppState {
    rpc_handler: Arc<RpcHandler>,
}

pub struct RpcServer {
    port: u16,
    handler: Arc<RpcHandler>,
}

impl RpcServer {
    pub fn new(blockchain: Arc<RwLock<Blockchain>>, chain_id: ChainId, port: u16) -> Self {
        let handler = Arc::new(RpcHandler::new(blockchain, chain_id));
        Self { port, handler }
    }

    pub async fn start(self) -> Result<(), Box<dyn std::error::Error>> {
        let addr = format!("0.0.0.0:{}", self.port);
        info!("🌐 Starting RPC server on http://{}", addr);

        let state = AppState {
            rpc_handler: self.handler,
        };

        // CORS layer to allow requests from anywhere
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        let app = Router::new()
            .route("/", post(handle_rpc))
            .layer(cors)
            .with_state(state);

        let listener = tokio::net::TcpListener::bind(&addr).await?;
        info!("✅ RPC server listening on http://{}", addr);

        axum::serve(listener, app).await?;

        Ok(())
    }
}

async fn handle_rpc(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    // Parse request
    let request: JsonRpcRequest = match serde_json::from_value(payload) {
        Ok(req) => req,
        Err(e) => {
            error!("Failed to parse RPC request: {}", e);
            let error_response = JsonRpcResponse::error(
                serde_json::Value::Null,
                ERROR_PARSE,
                format!("Parse error: {}", e),
            );
            return (StatusCode::OK, Json(error_response));
        }
    };

    // Handle request
    let response = state.rpc_handler.handle_request(request).await;

    (StatusCode::OK, Json(response))
}
