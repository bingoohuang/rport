use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use webrtc::ice_transport::ice_server::RTCIceServer;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IceServerConfig {
    pub urls: Vec<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl Default for IceServerConfig {
    fn default() -> Self {
        Self {
            urls: vec!["stun:restsend.com:3478".to_string()],
            username: None,
            password: None,
        }
    }
}

impl From<IceServerConfig> for RTCIceServer {
    fn from(config: IceServerConfig) -> Self {
        RTCIceServer {
            urls: config.urls,
            username: config.username.unwrap_or_default(),
            credential: config.password.unwrap_or_default(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RportConfig {
    pub id: Option<String>,
    pub server: Option<String>,
    pub token: Option<String>,
    pub ice_servers: Option<Vec<IceServerConfig>>,
}

impl Default for RportConfig {
    fn default() -> Self {
        Self {
            id: None,
            server: None,
            token: None,
            ice_servers: Some(vec![IceServerConfig::default()]),
        }
    }
}

impl RportConfig {
    pub fn load_from_file(path: &PathBuf) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(path)?;
        let config: RportConfig = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn load_default() -> Result<Self> {
        let home_dir =
            home::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;
        let config_path = home_dir.join(".rport.toml");
        Self::load_from_file(&config_path)
    }

    pub fn merge_with_cli(
        &mut self,
        cli_token: Option<String>,
        cli_server: Option<String>,
        cli_id: Option<String>,
    ) {
        if let Some(token) = cli_token {
            self.token = Some(token);
        }
        if let Some(server) = cli_server {
            self.server = Some(server);
        }
        if let Some(id) = cli_id {
            self.id = Some(id);
        }
    }

    pub async fn get_ice_servers(&self) -> Vec<RTCIceServer> {
        if let Some(ice_servers) = &self.ice_servers {
            ice_servers.iter().map(|s| s.clone().into()).collect()
        } else {
            let url = format!(
                "{}/rport/iceservers?token={}",
                self.server.as_deref().unwrap_or(""),
                self.token.as_deref().unwrap_or("")
            );
            let response = match Client::new().get(&url).send().await {
                Ok(resp) => resp,
                Err(_) => {
                    return vec![IceServerConfig::default().into()];
                }
            };

            if !response.status().is_success() {
                return vec![IceServerConfig::default().into()];
            }
            response
                .json()
                .await
                .unwrap_or_else(|_| vec![IceServerConfig::default().into()])
        }
    }
}
