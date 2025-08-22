use anyhow::Result;
use std::{collections::HashMap};

use crate::{interactions::{client_table::ClientTableRecord, shared::command_in_docker}};

pub async fn get_users() -> Result<Vec<ClientTableRecord>> {
    let output = command_in_docker(&["cat", "/opt/amnezia/awg/clientsTable"]).await?;
    let data = String::from_utf8_lossy(&output.stdout);
    let clients: Vec<ClientTableRecord> = serde_json::from_str(&data)?;
    Ok(clients)
}

pub async fn get_users_map() -> Result<HashMap<String, ClientTableRecord>> {
    let users = get_users().await?;
    Ok(users.into_iter().map(|u| (u.client_id.clone(), u)).collect())
}
