use anyhow::{anyhow, Result};
use bytes::Bytes;
use reqwest::Client;
use rport_common::*;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info};
use webrtc::data_channel::data_channel_init::RTCDataChannelInit;
use webrtc::data_channel::RTCDataChannel;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;

use crate::sdp_utils::strip_ipv6_candidates;
use crate::webrtc_config::WebRTCConfig;

pub struct CliClient {
    server_url: String,
    token: String,
    client: Client,
    webrtc_config: WebRTCConfig,
}

impl CliClient {
    pub fn new(server_url: String, token: String, webrtc_config: WebRTCConfig) -> Self {
        Self {
            server_url,
            token,
            client: Client::new(),
            webrtc_config,
        }
    }

    pub async fn list_agents(&self) -> Result<Vec<String>> {
        let url = format!("{}/rport/list?token={}", self.server_url, self.token);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to list agents: {}", response.status()));
        }

        let body: Value = response.json().await?;
        let agents = body["agents"]
            .as_array()
            .ok_or_else(|| anyhow!("Invalid response format"))?;

        Ok(agents
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect())
    }

    pub async fn connect_proxy_command(&self, agent_id: String) -> Result<()> {
        // ProxyCommand mode - NO LOGGING to avoid SSH interference

        // Create WebRTC connection (silently)
        let (peer_connection, data_channel) =
            self.create_webrtc_connection_silent(&agent_id).await?;

        // Wait for connection to be established (silently)
        self.wait_for_peer_connection_connected_silent(&peer_connection)
            .await?;
        self.wait_for_data_channel_open_silent(&data_channel)
            .await?;
        let cancel_token = tokio_util::sync::CancellationToken::new();
        // Set up stdin -> WebRTC forwarding
        let data_channel_write = data_channel.clone();
        let stdin_task = async move {
            let mut stdin = tokio::io::stdin();
            let mut buffer = [0u8; 4096];

            loop {
                match stdin.read(&mut buffer).await {
                    Ok(0) => {
                        // stdin closed - exit silently
                        break;
                    }
                    Ok(n) => {
                        let data = Bytes::from(buffer[..n].to_vec());
                        if data_channel_write.send(&data).await.is_err() {
                            // WebRTC send failed - exit silently
                            break;
                        }
                    }
                    Err(_) => {
                        // stdin read failed - exit silently
                        break;
                    }
                }
            }
        };

        // Set up WebRTC -> stdout forwarding
        let (stdout_tx, mut stdout_rx) = tokio::sync::mpsc::unbounded_channel();
        data_channel.on_message(Box::new(move |msg| {
            let stdout_tx = stdout_tx.clone();
            Box::pin(async move {
                let _ = stdout_tx.send(msg.data); // Ignore errors silently
            })
        }));

        let stdout_task = tokio::spawn(async move {
            let mut stdout = tokio::io::stdout();
            while let Some(data) = stdout_rx.recv().await {
                if stdout.write_all(&data).await.is_err() {
                    // stdout write failed - exit silently
                    break;
                }
                if stdout.flush().await.is_err() {
                    // stdout flush failed - exit silently
                    break;
                }
            }
        });

        // Wait for either task to complete (silently)
        tokio::select! {
            _ = cancel_token.cancelled() => {}
            _ = stdin_task => {}
            _ = stdout_task => {}
        }

        Ok(())
    }

    pub async fn connect_port_forward(&self, agent_id: String, local_port: u16) -> Result<()> {
        info!(
            "Starting port forward from localhost:{} to agent {}",
            local_port, agent_id
        );

        let listener = TcpListener::bind(format!("127.0.0.1:{}", local_port)).await?;
        info!("Listening on localhost:{}", local_port);

        loop {
            match listener.accept().await {
                Ok((tcp_stream, addr)) => {
                    info!("New connection from {}", addr);

                    let agent_id = agent_id.clone();
                    let client = self.clone();

                    tokio::spawn(async move {
                        if let Err(e) = client.handle_tcp_connection(tcp_stream, agent_id).await {
                            error!("Failed to handle TCP connection: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }

    async fn handle_tcp_connection(&self, tcp_stream: TcpStream, agent_id: String) -> Result<()> {
        // Create WebRTC connection for this TCP connection
        let (peer_connection, data_channel) = self.create_webrtc_connection(&agent_id).await?;

        // Wait for connection to be established
        self.wait_for_peer_connection_connected(&peer_connection)
            .await?;
        self.wait_for_data_channel_open(&data_channel).await?;

        info!("WebRTC connection established for TCP client");

        // Split the TCP stream
        let (mut tcp_read, mut tcp_write) = tcp_stream.into_split();

        let data_channel_write = data_channel.clone();

        let tcp_to_webrtc = async move {
            let mut buffer = [0u8; 4096];

            loop {
                match tcp_read.read(&mut buffer).await {
                    Ok(0) => {
                        info!("TCP connection closed");
                        break;
                    }
                    Ok(n) => {
                        let data = Bytes::from(buffer[..n].to_vec());
                        if let Err(e) = data_channel_write.send(&data).await {
                            error!("Failed to send data through WebRTC: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Failed to read from TCP: {}", e);
                        break;
                    }
                }
            }
        };

        // Set up WebRTC -> TCP forwarding
        let (webrtc_tx, mut webrtc_rx) = tokio::sync::mpsc::unbounded_channel();
        data_channel.on_message(Box::new(move |msg| {
            let webrtc_tx = webrtc_tx.clone();
            Box::pin(async move {
                if let Err(e) = webrtc_tx.send(msg.data) {
                    error!("Failed to send message to TCP channel: {}", e);
                }
            })
        }));

        let webrtc_to_tcp = async move {
            while let Some(data) = webrtc_rx.recv().await {
                if let Err(e) = tcp_write.write_all(&data).await {
                    error!("Failed to write to TCP: {}", e);
                    break;
                }
                if let Err(e) = tcp_write.flush().await {
                    error!("Failed to flush TCP: {}", e);
                    break;
                }
            }
        };

        // Wait for either direction to close
        tokio::select! {
            _ = tcp_to_webrtc => {
                info!("TCP to WebRTC forwarding ended");
            }
            _ = webrtc_to_tcp => {
                info!("WebRTC to TCP forwarding ended");
            }
        }

        Ok(())
    }

    async fn create_webrtc_connection(
        &self,
        agent_id: &str,
    ) -> Result<(Arc<RTCPeerConnection>, Arc<RTCDataChannel>)> {
        info!("Creating WebRTC peer connection for agent: {}", agent_id);

        // Create WebRTC peer connection
        let peer_connection = self.create_peer_connection().await?;

        peer_connection.on_peer_connection_state_change(Box::new(
            move |s: RTCPeerConnectionState| {
                info!("Peer Connection State has changed: {}", s);
                Box::pin(async move {})
            },
        ));

        // Create a data channel before creating the offer
        let data_channel_config = RTCDataChannelInit {
            ordered: Some(true),
            max_retransmits: Some(10),
            ..Default::default()
        };
        let data_channel = peer_connection
            .create_data_channel("port-forward", Some(data_channel_config))
            .await?;

        info!("Data channel created on client side");

        // Add data channel state monitoring
        data_channel.on_open(Box::new(move || {
            Box::pin(async move {
                info!("✅ Data channel opened on client side!");
            })
        }));

        data_channel.on_close(Box::new(move || {
            Box::pin(async move {
                info!("❌ Data channel closed on client side!");
            })
        }));

        data_channel.on_error(Box::new(move |err| {
            Box::pin(async move {
                error!("💥 Data channel error on client side: {:?}", err);
            })
        }));

        // Create offer
        let offer = peer_connection.create_offer(None).await?;
        peer_connection.set_local_description(offer.clone()).await?;

        // Wait for ICE gathering
        peer_connection
            .gathering_complete_promise()
            .await
            .recv()
            .await;

        info!("Client ICE candidate gathering done...");
        let offer = peer_connection
            .local_description()
            .await
            .ok_or_else(|| anyhow!("Failed to get local description after ICE gathering"))?;

        // Strip IPv6 candidates from offer
        let filtered_offer_sdp = strip_ipv6_candidates(&offer.sdp);

        // Send offer to server
        let offer_msg = OfferMessage {
            id: agent_id.to_string(),
            offer: filtered_offer_sdp,
        };

        let url = format!("{}/rport/offer?token={}", self.server_url, self.token);
        let response = self.client.post(&url).json(&offer_msg).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to send offer: {}", response.status()));
        }

        let response_body: Value = response.json().await?;
        let answer_sdp = response_body["answer"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing answer in response"))?;

        // Strip IPv6 candidates from answer
        let filtered_answer_sdp = strip_ipv6_candidates(answer_sdp);

        // Set remote description from answer
        let answer = RTCSessionDescription::answer(filtered_answer_sdp)?;
        peer_connection.set_remote_description(answer).await?;

        info!("WebRTC handshake completed successfully");

        Ok((peer_connection, data_channel))
    }

    async fn create_webrtc_connection_silent(
        &self,
        agent_id: &str,
    ) -> Result<(Arc<RTCPeerConnection>, Arc<RTCDataChannel>)> {
        // Silent version for ProxyCommand mode - no logging

        // Create WebRTC peer connection
        let peer_connection = self.create_peer_connection().await?;

        // Create a data channel before creating the offer
        let data_channel_config = RTCDataChannelInit {
            ordered: Some(true),
            ..Default::default()
        };
        let data_channel = peer_connection
            .create_data_channel("port-forward", Some(data_channel_config))
            .await?;

        // Create offer
        let offer = peer_connection.create_offer(None).await?;
        peer_connection.set_local_description(offer.clone()).await?;

        // Wait for ICE gathering
        peer_connection
            .gathering_complete_promise()
            .await
            .recv()
            .await;

        let offer = peer_connection
            .local_description()
            .await
            .ok_or_else(|| anyhow!("Failed to get local description after ICE gathering"))?;

        // Strip IPv6 candidates from offer
        let filtered_offer_sdp = strip_ipv6_candidates(&offer.sdp);

        // Send offer to server
        let offer_msg = OfferMessage {
            id: agent_id.to_string(),
            offer: filtered_offer_sdp,
        };

        let url = format!("{}/rport/offer?token={}", self.server_url, self.token);
        let response = self.client.post(&url).json(&offer_msg).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to send offer: {}", response.status()));
        }

        let response_body: Value = response.json().await?;
        let answer_sdp = response_body["answer"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing answer in response"))?;

        // Strip IPv6 candidates from answer
        let filtered_answer_sdp = strip_ipv6_candidates(answer_sdp);

        // Set remote description from answer
        let answer = RTCSessionDescription::answer(filtered_answer_sdp)?;
        peer_connection.set_remote_description(answer).await?;

        Ok((peer_connection, data_channel))
    }

    async fn create_peer_connection(&self) -> Result<Arc<RTCPeerConnection>> {
        self.webrtc_config.create_peer_connection().await
    }

    async fn wait_for_peer_connection_connected(
        &self,
        peer_connection: &Arc<RTCPeerConnection>,
    ) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let tx = Arc::new(tokio::sync::Mutex::new(Some(tx)));

        let tx_clone = Arc::clone(&tx);
        peer_connection.on_peer_connection_state_change(Box::new(move |state| {
            let tx = Arc::clone(&tx_clone);
            Box::pin(async move {
                info!("Peer connection state changed: {:?}", state);
                if state == RTCPeerConnectionState::Connected {
                    if let Some(sender) = tx.lock().await.take() {
                        let _ = sender.send(());
                    }
                }
            })
        }));

        // Wait for connection with timeout
        tokio::select! {
            _ = rx => {
                info!("Peer connection established successfully");
                Ok(())
            }
            _ = tokio::time::sleep(Duration::from_secs(30)) => {
                Err(anyhow!("Timeout waiting for peer connection to connect"))
            }
        }
    }

    async fn wait_for_data_channel_open(&self, data_channel: &Arc<RTCDataChannel>) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let tx = Arc::new(tokio::sync::Mutex::new(Some(tx)));

        let tx_clone = Arc::clone(&tx);
        data_channel.on_open(Box::new(move || {
            let tx = Arc::clone(&tx_clone);
            Box::pin(async move {
                info!("Data channel opened on client side!");
                if let Some(sender) = tx.lock().await.take() {
                    let _ = sender.send(());
                }
            })
        }));

        // Wait with timeout
        tokio::select! {
            _ = rx => {
                info!("Data channel opened successfully");
                Ok(())
            }
            _ = tokio::time::sleep(Duration::from_secs(10)) => {
                Err(anyhow!("Timeout waiting for data channel to open"))
            }
        }
    }

    async fn wait_for_peer_connection_connected_silent(
        &self,
        peer_connection: &Arc<RTCPeerConnection>,
    ) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let tx = Arc::new(tokio::sync::Mutex::new(Some(tx)));

        let tx_clone = Arc::clone(&tx);
        peer_connection.on_peer_connection_state_change(Box::new(move |state| {
            let tx = Arc::clone(&tx_clone);
            Box::pin(async move {
                if state == RTCPeerConnectionState::Connected {
                    if let Some(sender) = tx.lock().await.take() {
                        let _ = sender.send(());
                    }
                }
            })
        }));

        // Wait for connection with timeout (silently)
        tokio::select! {
            _ = rx => Ok(()),
            _ = tokio::time::sleep(Duration::from_secs(30)) => {
                Err(anyhow!("Timeout waiting for peer connection to connect"))
            }
        }
    }

    async fn wait_for_data_channel_open_silent(
        &self,
        data_channel: &Arc<RTCDataChannel>,
    ) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let tx = Arc::new(tokio::sync::Mutex::new(Some(tx)));

        let tx_clone = Arc::clone(&tx);
        data_channel.on_open(Box::new(move || {
            let tx = Arc::clone(&tx_clone);
            Box::pin(async move {
                if let Some(sender) = tx.lock().await.take() {
                    let _ = sender.send(());
                }
            })
        }));

        // Wait with timeout (silently)
        tokio::select! {
            _ = rx => Ok(()),
            _ = tokio::time::sleep(Duration::from_secs(10)) => {
                Err(anyhow!("Timeout waiting for data channel to open"))
            }
        }
    }
}

impl Clone for CliClient {
    fn clone(&self) -> Self {
        Self {
            server_url: self.server_url.clone(),
            token: self.token.clone(),
            client: Client::new(),
            webrtc_config: self.webrtc_config.clone(),
        }
    }
}
