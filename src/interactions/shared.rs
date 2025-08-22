use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use chrono::{NaiveDate, NaiveDateTime};
use serde::{Serialize, Deserialize};
use tokio::{process::Command, sync::RwLock};
use uuid::Uuid;

use crate::{interactions::{cfg::{self, drop_all, rm_by_id}, client_table::ClientTableRecord, get::get_users_map, pages::set_page}, ENV};

pub async fn sync_wg() -> Result<std::process::Output> {
    command_in_docker(&["bash", "-c", "wg syncconf wg0 <(wg-quick strip /opt/amnezia/awg/wg0.conf)"]).await
}


pub async fn write_to_docker(data: &str, dst: &str) -> Result<()> {
    let tmp_id = Uuid::new_v4().simple().to_string();
    tokio::fs::write(format!("/tmp/{}", tmp_id), data).await?;
    copy_to_docker(&format!("/tmp/{}", tmp_id), dst).await?;
    shred(&format!("/tmp/{}", tmp_id)).await?;
    Ok(())
}

pub async fn command_in_docker(args: &[&str]) -> Result<std::process::Output> {
    let mut cmd = Command::new("docker");
    cmd.args(["exec", "-i", &ENV.container]);
    for arg in args {
        cmd.arg(arg);
    };
    Ok(cmd.output().await?)
}

pub async fn copy_to_docker(src: &str, dst: &str) -> Result<std::process::ExitStatus> {
    let mut cmd = Command::new("docker");
    cmd.args([
            "cp",
            src,
            &format!("{}:{}", ENV.container, dst),
        ]);
    Ok(cmd.status().await?)
}

pub async fn shred(src: &str) -> Result<std::process::ExitStatus> {
    let mut cmd = Command::new("shred");
    cmd.args(["-u", src]);
    Ok(cmd.status().await?)
}

#[derive(Clone, Default)]
pub struct AppState {
    pub stored: Arc<RwLock<StoredUsers>>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct StoredUsers {
    records: HashMap<String, ClientTableRecord>,
    pages: HashMap<String, HashMap<String, (String, String)>>,
    id_to_group: HashMap<String, String>,
    group_to_guid: HashMap<String, String>,
}

impl AppState {
    pub fn new() -> Self {
        if let Ok(b) =std::fs::read(&ENV.stored_file) {
            if let Ok((users, _)) = bincode::serde::decode_from_slice(&b, bincode::config::standard()) {
                Self {stored: Arc::new(RwLock::new(users))}
            } else {
                Default::default()
            }
        } else {
            Default::default()
        }
    }

    async fn backup(u: &StoredUsers) {
        let b = bincode::serde::encode_to_vec(&u, bincode::config::standard()).unwrap();
        tokio::fs::write(&ENV.stored_file, b).await.ok();
    }

    pub async fn fetch_users(&self) -> Result<()> {
        let users = get_users_map().await?;
        self.stored.write().await.records = users;
        Ok(())
    }
    
    pub async fn rm_by_id(&self, client_id: &str) -> Result<()> {
        let mut s = self.stored.write().await;
        rm_by_id(client_id).await?;
        tracing::info!("Waiting for lock: {}", client_id);
        tracing::info!("Got lock: {}", client_id);
        s.records.remove(client_id);
        if let Some(group) = s.id_to_group.remove(client_id) {
            s.pages.remove(&group);
            if let Some(guid) = s.group_to_guid.remove(&group) {
                if let Some(configs) = s.pages.get_mut(&group) {
                    configs.remove(client_id);
                    set_page(&guid, configs).await;
                };
            }
        }
        Self::backup(&*s).await;
        drop(s);
        self.fetch_users().await.ok();
        Ok(())
    }

    pub async fn add_user(&self, name: &str, group: String) -> Result<GroupRecord> {
        let mut s = self.stored.write().await;
        let r = Self::add_user_raw(&mut s, name, group).await?;
        Self::backup(&*s).await;
        drop(s);
        self.fetch_users().await.ok();
        Ok(r)
    }

    async fn add_user_raw(s: &mut StoredUsers, name: &str, group: String) -> Result<GroupRecord> {
        let (public_id, config) = cfg::create_user(name).await?;
        s.pages.entry(group.to_string()).or_default().insert(public_id.clone(), (name.to_string(), config));
        tracing::info!("Pages: {:#?}", s.pages);

        s.id_to_group.insert(public_id.clone(), group.to_string());
        let guid = if let Some(k) = s.group_to_guid.get(&group) {
            k.clone()
        } else {
            let guid = Uuid::new_v4().simple().to_string();
            s.group_to_guid.insert(group.to_string(), guid.clone());
            guid
        };

        if let Some(configs) = s.pages.get(&group) {
            set_page(&guid, configs).await;
        };
        Ok(GroupRecord{guid, group})
    }

    pub async fn add_users(&self, batch: Vec<(String, String)>) -> Result<Vec<GroupRecord>> {
        let mut s = self.stored.write().await;
        let mut res = Vec::with_capacity(batch.len());
        for user in batch {
            if let Ok(r) = Self::add_user_raw(&mut s, &user.0, user.1).await {
                res.push(r);
            }
        }
        Self::backup(&*s).await;
        drop(s);
        self.fetch_users().await.ok();
        Ok(res)
    }




    pub async fn group_records(&self) -> Vec<GroupRecord>{
        self.stored.read().await.group_to_guid.clone().into_iter().map(|(group, guid)| GroupRecord{group, guid}).collect()
    }

    pub async fn user_list(&self) -> Vec<User> {
        self.stored.read().await.records.iter().map(|c| c.1.into()).collect()
    }

    pub async fn user_stats(&self) -> Vec<UserStats> {
        // todo: caching?
        self.stored.read().await.records.iter().map(|c| c.1.into()).collect()
    }

    pub async fn clear(&self) {
        let mut s = self.stored.write().await;
        drop_all().await.ok();
        *s = StoredUsers::default();
        tokio::fs::remove_dir_all("data/served").await.ok();
        Self::backup(&*s).await;
        drop(s);

    }
}

#[derive(Serialize)]
pub struct GroupRecord {group: String, guid: String}


#[derive(Serialize)]
pub struct User {
    pub uid: String,
    pub name: String
}

impl From<&ClientTableRecord> for User {
    fn from(record: &ClientTableRecord) -> Self {
        Self {
            uid: record.client_id.clone(),
            name: record.user_data.client_name.clone()
        }
    }
}

#[derive(Serialize)]
pub struct UserStats {
    uid: String,
    name: String,
    recv: String,
    sent: String,
    last_seen: String,
    created: String
}

impl From<&ClientTableRecord> for UserStats {
    fn from(record: &ClientTableRecord) -> Self {
        Self {
            uid: record.client_id.clone(),
            name: record.user_data.client_name.clone(),
            recv: record.user_data.data_received.clone().unwrap_or("0 KiB".to_string()),
            sent: record.user_data.data_sent.clone().unwrap_or("0 KiB".to_string()),
            last_seen: record.user_data.latest_handshake.clone().unwrap_or("Never".to_string()),
            created: record.user_data.creation_date.clone()
        }
    }
}

