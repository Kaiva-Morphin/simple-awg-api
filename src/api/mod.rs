use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use tracing::error;

use crate::interactions::{shared::AppState, wg0::AwgInterfaceConf};



pub async fn user_list(
    State(state): State<AppState>,
) -> impl IntoResponse {
    Json(state.user_list().await)
} 

#[derive(Deserialize)]
pub struct CreateRequest {
    name: String,
    group: String
}

pub async fn create_user(
    State(state): State<AppState>,
    Json(CreateRequest{name, group}): Json<CreateRequest>,
) -> impl IntoResponse {
    match state.add_user(&name, group).await {
        Ok(r) => {
            Json(r).into_response()
        }
        Err(e) => {
            error!("{:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn create_users(
    State(state): State<AppState>,
    Json(batch): Json<Vec<CreateRequest>>,
) -> impl IntoResponse {
    match state.add_users(batch.into_iter().map(|c| (c.name, c.group)).collect()).await {
        Ok(r) => {
            Json(r).into_response()
        }
        Err(e) => {
            error!("{:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[axum::debug_handler]
pub async fn delete_user(
    State(state): State<AppState>,
    Json(client_id): Json<String>,
) -> impl IntoResponse {
    if let Err(e) = state.rm_by_id(&client_id).await {
        error!("{:?}", e);
        (StatusCode::INTERNAL_SERVER_ERROR).into_response()
    } else {
        (StatusCode::OK).into_response()
    } 
} 

pub async fn users_stats(
    State(state): State<AppState>,
) -> impl IntoResponse {
    Json(state.user_stats().await)
}

pub async fn groups(
    State(state): State<AppState>,
) -> impl IntoResponse {
    Json(state.group_records().await)
}


pub async fn clear(
    State(state): State<AppState>,
) -> impl IntoResponse {
    state.clear().await;
    (StatusCode::OK).into_response()
}

pub async fn last_id(
) -> impl IntoResponse {
    let r = AwgInterfaceConf::from_docker().await.unwrap().unwrap();
    let r = (StatusCode::OK, Json(r.get_last_id())).into_response();
    r
}