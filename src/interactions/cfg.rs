use std::{collections::HashMap, path::PathBuf};

use handlebars::Handlebars;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use anyhow::{Ok, Result};
use tokio::fs;
use tracing::{info, warn};
use chrono::prelude::*;

use crate::{interactions::{client_table::{get_client_table_from_docker, ClientTableRecord, ClientTableRecordUserData}, shared::{command_in_docker, copy_to_docker, shred, sync_wg, write_to_docker}, wg0::{AWGInterfaceData,  AwgInterfaceConf, AwgPeer}}, ENV};






pub async fn rm_by_id(client_id: &str) -> Result<()> {
    let mut clients_table = get_client_table_from_docker().await?;
    clients_table.retain(|c| c.client_id != client_id);
    let Some(mut wg_conf) = AwgInterfaceConf::from_docker().await? else {
        return Err(anyhow::anyhow!("Failed to parse wg0.conf"));
    };
    wg_conf.peers.remove(client_id);
    write_to_docker(&serde_json::to_string_pretty(&clients_table)?, "/opt/amnezia/awg/clientsTable").await?;
    write_to_docker(&wg_conf.to_string(), "/opt/amnezia/awg/wg0.conf").await?;
    sync_wg().await?;

    Ok(())
}

pub async fn create_user(name: &str) -> Result<(String, String)> {
    let mut wg = AwgInterfaceConf::from_docker().await?
        .ok_or(anyhow::anyhow!("Failed to parse wg0.conf"))?;
    info!("Wg interface: {}", wg.interface);
    let it_data = AWGInterfaceData::from_docker().await?.ok_or(anyhow::anyhow!("Failed to get wg0 data"))?;
    let mut clients_table = get_client_table_from_docker().await?;

    let _o = command_in_docker(
        &[
            "bash", "-c", r#"cd /opt/amnezia/awg \
            && umask 077 \
            && wg genkey | tee client.key | wg pubkey > client.pub \
            && wg genpsk > client.psk \
            && rm -f /tmp/client \
            && cat client.pub >> /tmp/client \
            && cat client.key >> /tmp/client \
            && cat client.psk >> /tmp/client \
            && cat /tmp/client \
            && rm -f /tmp/client && rm -f client.key && rm -f client.pub"#
        ]
    ).await?;
    
    info!("Res: {:?}", String::from_utf8_lossy(&_o.stderr));
    let r = String::from_utf8_lossy(&_o.stdout);
    let mut i = r.split("\n");
    let public = i.next().ok_or(anyhow::anyhow!("No public key generated"))?;
    let private = i.next().ok_or(anyhow::anyhow!("No private key generated"))?;
    let psk = i.next().ok_or(anyhow::anyhow!("No preshared generated"))?;

    let cfg = ClientConfig {
        addr: format!("{}{}", ENV.mask, it_data.last_id + 1),
        dns: ENV.dns.clone(),
        private_key: private.to_string(),
        jc: it_data.jc,
        jmin: it_data.jmin,
        jmax: it_data.jmax,
        s1: it_data.s1,
        s2: it_data.s2,
        h1: it_data.h1,
        h2: it_data.h2,
        h3: it_data.h3,
        h4: it_data.h4,
        peer_public_key: it_data.public_key,
        peer_preshared_key: psk.to_string(),

        peer_allowed_ips: ENV.host.clone(),
        peer_endpoint: format!("{}:{}", ENV.host, it_data.port),
        peer_persistent_keepalive: ENV.keepalive.clone()
    };
    let rendered = cfg.render()?;
    let peer = cfg.to_peer(public.to_string());
    let record = cfg.to_record(name.to_string(), public.to_string());
    wg.peers.insert(public.to_string(), peer);
    clients_table.push(record);

    write_to_docker(&serde_json::to_string_pretty(&clients_table)?, "/opt/amnezia/awg/clientsTable").await?;
    write_to_docker(&wg.to_string(), "/opt/amnezia/awg/wg0.conf").await?;
    sync_wg().await?;
    info!("Created user: {}", name);
    Ok((public.to_string(), rendered))
}


#[derive(Serialize)]
pub struct ClientConfig {
    pub addr: String,
    pub dns: String,
    pub private_key: String,
    pub jc: u32,
    pub jmin: u32,
    pub jmax: u32,
    pub s1: u32,
    pub s2: u32,
    pub h1: u32,
    pub h2: u32,
    pub h3: u32,
    pub h4: u32,

    pub peer_public_key: String,
    pub peer_preshared_key: String,
    pub peer_allowed_ips: String,
    pub peer_endpoint: String,
    pub peer_persistent_keepalive: String
}


impl ClientConfig {
    fn render(&self) -> Result<String> {
        let mut handlebars = Handlebars::new();
        handlebars.register_template_file("config", "data/templates/config.hbs")?;
        let r = handlebars.render("config",self);
        Ok(r?)
    }

    fn to_record(&self, name: String, public: String) -> ClientTableRecord {
        let now: DateTime<Local> = Local::now();
        let formatted = now.format("%a %b %d %H:%M:%S %Y").to_string();
        ClientTableRecord {
            client_id: public.clone(),
            user_data: ClientTableRecordUserData {
                client_name: name.to_string(),
                data_received: None,
                data_sent: None,
                latest_handshake: None,
                allowed_ips: None,
                creation_date: formatted
            }
        }
    }

    fn to_peer(&self, client_pub: String) -> AwgPeer {
        AwgPeer {
            public_key: client_pub,
            preshared_key: self.peer_preshared_key.clone(),
            allowed_ips: format!("{}/32", self.addr),
        }
    }
}

pub async fn drop_all() -> Result<()> {
    let clients_table: Vec<ClientTableRecord> = Vec::new();
    let Some(mut wg_conf) = AwgInterfaceConf::from_docker().await? else {
        return Err(anyhow::anyhow!("Failed to parse wg0.conf"));
    };
    wg_conf.peers = HashMap::new();
    write_to_docker(&serde_json::to_string_pretty(&clients_table)?, "/opt/amnezia/awg/clientsTable").await?;
    write_to_docker(&wg_conf.to_string(), "/opt/amnezia/awg/wg0.conf").await?;
    sync_wg().await?;
    Ok(())
}