use anyhow::Result;
use reqwest::Client;
use std::sync::Arc;
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::RTCPeerConnection;

use crate::config::IceServerConfig;

#[derive(Clone)]
pub struct WebRTCConfig {
    pub server: String,
    pub token: String,
    pub ice_servers: Vec<IceServerConfig>,
}

impl WebRTCConfig {
    pub fn new(server: String, token: String, ice_servers: Vec<IceServerConfig>) -> Self {
        Self {
            server,
            token,
            ice_servers,
        }
    }

    pub async fn get_ice_servers(&self) -> Vec<RTCIceServer> {
        if self.ice_servers.len() > 0 {
            return self
                .ice_servers
                .clone()
                .into_iter()
                .map(|c| c.into())
                .collect();
        }
        let url = format!("{}/rport/iceservers?token={}", self.server, self.token);
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
            .json::<Vec<IceServerConfig>>()
            .await
            .map(|configs| configs.into_iter().map(|c| c.into()).collect())
            .unwrap_or_default()
    }

    pub async fn create_peer_connection(&self) -> Result<Arc<RTCPeerConnection>> {
        let api = APIBuilder::new().build();
        let config = RTCConfiguration {
            ice_servers: self.get_ice_servers().await,
            ..Default::default()
        };
        tracing::info!("Using ICE servers: {:?}", config.ice_servers);
        let peer_connection = Arc::new(api.new_peer_connection(config).await?);
        Ok(peer_connection)
    }
}
