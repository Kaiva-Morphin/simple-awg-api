use std::fmt::Result;

use serde::{Deserialize, Serialize};

use crate::interactions::shared::command_in_docker;
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ClientTableRecord {
    #[serde(rename = "clientId")]
    pub client_id: String,

    #[serde(rename = "userData")]
    pub user_data: ClientTableRecordUserData,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ClientTableRecordUserData {
    #[serde(rename = "clientName")]
    pub client_name: String,
    #[serde(rename = "creationDate")]
    pub creation_date: String,
    #[serde(rename = "dataReceived", skip_serializing_if = "Option::is_none")]
    pub data_received: Option<String>,
    #[serde(rename = "dataSent", skip_serializing_if = "Option::is_none")]
    pub data_sent: Option<String>,
    #[serde(rename = "latestHandshake", skip_serializing_if = "Option::is_none")]
    pub latest_handshake: Option<String>,
    #[serde(rename = "allowedIps", skip_serializing_if = "Option::is_none")]
    pub allowed_ips: Option<String>,
}


pub async fn get_client_table_from_docker() -> anyhow::Result<Vec<ClientTableRecord>> {
    let output = command_in_docker(&["cat", "/opt/amnezia/awg/clientsTable"]).await?;
    let data = String::from_utf8_lossy(&output.stdout);
    let clients: Vec<ClientTableRecord> = serde_json::from_str(&data)?;
    Ok(clients)
} 