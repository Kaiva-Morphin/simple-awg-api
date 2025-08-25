use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

use crate::interactions::shared::command_in_docker;

#[derive(Debug)]
pub struct AwgInterfaceConf {
    pub interface: String,
    pub public_key: String,
    pub parsed_iface: AWGInterfaceData,
    pub peers: HashMap<String, AwgPeer>,
}

#[derive(Debug)]
pub struct AwgPeer {
    pub public_key: String,
    pub preshared_key: String,
    pub allowed_ips: String
}

impl AwgPeer {
    pub fn parse_str(lines: &Vec<String>) -> Option<Self> {
        let mut public_key = None;
        let mut preshared_key = None;
        let mut allowed_ips = None;
        for line in lines {
            if line.starts_with("PublicKey") {
                public_key = Some(line.split_once('=')?.1.trim().to_string());
            } else if line.starts_with("PresharedKey") {
                preshared_key = Some(line.split_once('=')?.1.trim().to_string());
            } else if line.starts_with("AllowedIPs") {
                allowed_ips = Some(line.split_once('=')?.1.trim().to_string());
            } else {
                error!("Unknown line: {}", line);
            }
        }
        Some(AwgPeer {
            public_key: public_key?,
            preshared_key: preshared_key?,
            allowed_ips: allowed_ips?
        })
    }

    pub fn to_string(&self) -> String {
        format!("[Peer]\nPublicKey = {}\nPresharedKey = {}\nAllowedIPs = {}", self.public_key, self.preshared_key, self.allowed_ips)
    }
}



impl AwgInterfaceConf {
    fn try_parse_id(p: &str) -> Option<u32> {
        let id = p
            .split('/')
            .next()?
            .rsplit('.')
            .next()?
            .parse()
            .ok()?
            ;
        Some(id)
    }
    pub fn get_last_id(&self) -> u32 {
        let mut last : u32 = 1;
        for (_, peer) in self.peers.iter() {
            let Some(id) = Self::try_parse_id(&peer.allowed_ips) else {
                warn!("Failed to parse id: {}", peer.allowed_ips);
                continue;
            };
            last = last.max(id);
        }
        last
    }
    pub fn to_string(&self) -> String {
        format!("[Interface]\n{}\n\n{}\n\n", self.interface, self.peers.values().map(|p| p.to_string()).collect::<Vec<String>>().join("\n\n"))
    }



    pub async fn from_docker() -> anyhow::Result<Option<Self>> {
        let output = command_in_docker(&["cat", "/opt/amnezia/awg/wg0.conf"]).await?;
        info!("Got wg0.conf");
        let data = String::from_utf8_lossy(&output.stdout);
        let pk = command_in_docker(&["cat", "/opt/amnezia/awg/wireguard_server_public_key.key"]).await?;
        info!("Got public key");
        let public_key = String::from_utf8_lossy(&pk.stdout).into_owned();
        

        let mut interface_lines: Vec<String> = Vec::new();
        let mut peers = HashMap::new();

        let mut current_section = String::new();
        let mut current_lines: Vec<String> = Vec::new();

        let mut store_section = |section: &str, lines: &Vec<String>| {
            if !section.is_empty() {
                match section {
                    "Interface" => interface_lines = lines.clone(),
                    "Peer" => {
                        if let Some(peer) = AwgPeer::parse_str(lines) {
                            peers.insert(peer.public_key.clone(), peer);
                        } else {
                            warn!("Failed to parse peer section!");
                        }
                        
                    }
                    _ => {}
                }
            }
        };

        for line in data.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                store_section(&current_section, &current_lines);
                current_section = line[1..line.len()-1].to_string();
                current_lines.clear();
            } else {
                current_lines.push(line.to_string());
            }
        }
        store_section(&current_section, &current_lines);
        
        if interface_lines.is_empty() {
            return Ok(None)
        }

        let interface = interface_lines.join("\n");
        Ok(Some(Self {
            public_key,
            parsed_iface: AWGInterfaceData::from_str(&interface).ok_or(anyhow::anyhow!("Failed to parse wg0.conf"))?,
            interface,
            peers,
        }))
    }
}


#[allow(unused)]
#[derive(Debug, Deserialize, Clone)]
pub struct AWGInterfaceData {
    pub port: String,
    pub jc: String,
    pub jmin: String,
    pub jmax: String,
    pub s1: String,
    pub s2: String,
    pub h1: String,
    pub h2: String,
    pub h3: String,
    pub h4: String
}

#[derive(Default)]
struct AWGRaw<'a> {
    pub map: HashMap<&'a str, &'a str>,
    pub last_allowed_ip: Option<String>,
    pub public_key: Option<String>,
    pub port: Option<String>
}

impl AWGInterfaceData {
    // pub async fn from_docker() -> anyhow::Result<Option<Self>> {
    //     todo!("deprecated");
    //     let r = command_in_docker(&["wg"]).await?;
    //     let data = String::from_utf8_lossy(&r.stdout);
    //     Ok(Some(AWGInterfaceData::from_str(&data).ok_or(anyhow::anyhow!("Failed to parse data"))?))
    // } 
    fn raw_from_str(s: &str) -> AWGRaw {
        let mut raw = AWGRaw::default();
        
        raw
    }

    pub fn from_str(s: &str) -> Option<Self> {
        let raw = Self::raw_from_str(s);

        // let mut last_allowed_ip = None;
        // let mut public_key = None;
        // let mut port = None;
        let mut map = HashMap::new();

        for line in s.lines() {
            let line = line.trim();
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value = value.trim();
                map.insert(key.to_lowercase(), value);
            }
        }

        let port = raw.port?;
        Some(AWGInterfaceData {
            port,
            jc: raw.map.get("jc")?.parse().ok()?,
            jmin: raw.map.get("jmin")?.parse().ok()?,
            jmax: raw.map.get("jmax")?.parse().ok()?,
            s1: raw.map.get("s1")?.parse().ok()?,
            s2: raw.map.get("s2")?.parse().ok()?,
            h1: raw.map.get("h1")?.parse().ok()?,
            h2: raw.map.get("h2")?.parse().ok()?,
            h3: raw.map.get("h3")?.parse().ok()?,
            h4: raw.map.get("h4")?.parse().ok()?,
        })
    }
}
