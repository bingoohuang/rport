use anyhow::Result;
use std::sync::Arc;
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::RTCPeerConnection;

#[derive(Clone)]
pub struct WebRTCConfig {
    pub ice_servers: Vec<RTCIceServer>,
}

impl WebRTCConfig {
    pub fn new(ice_servers: Vec<RTCIceServer>) -> Self {
        Self { ice_servers }
    }

    pub async fn create_peer_connection(&self) -> Result<Arc<RTCPeerConnection>> {
        let api = APIBuilder::new().build();
        let config = RTCConfiguration {
            ice_servers: self.ice_servers.clone(),
            ..Default::default()
        };

        let peer_connection = Arc::new(api.new_peer_connection(config).await?);
        Ok(peer_connection)
    }
}
