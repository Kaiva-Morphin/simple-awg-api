use anyhow::Result;
use once_cell::sync::OnceCell;
use tracing::*;
use axum::routing::{delete, get, post};
use crate::{api::*, interactions::shared::{sync_wg, AppState}, util::middleware};

mod util;
mod interactions;
mod api;

env_config!(
    ".env" => ENV = Env {
        container: String = "amnezia-awg".to_string(),
        addr: String = "0.0.0.0:8080".to_string(),
        host: String,
        dns: String,
        keepalive: String,
        mask: String,
        stored_file: String,
    }
);


#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let state = AppState::new();
    state.fetch_users().await?;
    let router = axum::Router::new()
        .route("/users", get(user_list))
        .route("/users", delete(clear))
        .route("/users", post(create_users))
        .route("/stats", get(users_stats))
        .route("/user", post(create_user))
        .route("/user", delete(delete_user))
        .route("/groups", get(groups))
        .layer(axum::middleware::from_fn(layer_with_unique_span!("request ")))
        .layer(axum::middleware::from_fn(middleware::logging_middleware))
        .with_state(state);

    info!("Listening on {}", ENV.addr);
    let listener = tokio::net::TcpListener::bind(ENV.addr.clone()).await?;
    axum::serve(listener, router).await?;
    Ok(())
}
