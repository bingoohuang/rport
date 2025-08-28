use anyhow::{anyhow, Result};
use bytes::Bytes;
use futures::StreamExt;
use reqwest::Client;
use rport_common::{AnswerMessage, ServerMessage, RECONNECT_INTERVAL};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use webrtc::data_channel::RTCDataChannel;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;

use crate::webrtc_config::WebRTCConfig;
use crate::sdp_utils::strip_ipv6_candidates;

#[allow(dead_code)]
struct ConnectionSession {
    session_id: Uuid,
    client_ip: String,
    peer_connection: Arc<RTCPeerConnection>,
}

pub struct Agent {
    server_url: String,
    token: String,
    id: String,
    target_host: String,
    target_port: u16,
    client: Client,
    webrtc_config: WebRTCConfig,
}

impl Agent {
    pub fn new(
        server_url: String,
        token: String,
        id: String,
        target_host: String,
        target_port: u16,
        webrtc_config: WebRTCConfig,
    ) -> Self {
        Self {
            server_url,
            token,
            id,
            target_host,
            target_port,
            client: Client::new(),
            webrtc_config,
        }
    }

    pub async fn run(&self) -> Result<()> {
        info!(
            "Starting agent: {} on {}:{}",
            self.id, self.target_host, self.target_port
        );
        info!("Connecting to server: {}", self.server_url);
        info!(
            "Will forward connections to {}:{}",
            self.target_host, self.target_port
        );

        // Verify the target port is accessible (but don't require it to be ready)
        match TcpStream::connect(format!("{}:{}", self.target_host, self.target_port)).await {
            Ok(_) => {
                info!(
                    "✅ Successfully connected to {}:{}",
                    self.target_host, self.target_port
                );
            }
            Err(e) => {
                warn!(
                    "⚠️  Could not connect to {}:{}: {}. Service may not be running yet.",
                    self.target_host, self.target_port, e
                );
            }
        }

        loop {
            match self.register_and_listen().await {
                Ok(_) => {
                    info!("SSE connection ended normally");
                }
                Err(e) => {
                    error!("SSE connection failed: {}", e);
                }
            }

            info!("Reconnecting in {} seconds...", RECONNECT_INTERVAL);
            tokio::time::sleep(Duration::from_secs(RECONNECT_INTERVAL)).await;
        }
    }

    async fn register_and_listen(&self) -> Result<()> {
        let url = format!(
            "{}/rport/connect?token={}&id={}",
            self.server_url, self.token, self.id
        );
        info!("Connecting to: {}", url);

        // Use SSE connection instead of WebSocket
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to connect: {}", response.status()));
        }

        let mut stream = response.bytes_stream();
        let last_ping = tokio::sync::Mutex::new(tokio::time::Instant::now());
        let handle_stream = async {
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        *last_ping.lock().await = tokio::time::Instant::now();
                        let text = String::from_utf8_lossy(&bytes);
                        // Parse SSE events
                        for line in text.lines() {
                            if line.starts_with("data: ") {
                                let data = &line[6..]; // Remove "data: " prefix
                                if let Ok(server_msg) = serde_json::from_str::<ServerMessage>(data)
                                {
                                    if let Err(e) = self.handle_server_message(server_msg).await {
                                        error!("Failed to handle server message: {}", e);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("SSE stream error: {}", e);
                        break;
                    }
                }
            }
        };

        let check_has_ping_loop = async {
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                if last_ping.lock().await.elapsed() > Duration::from_secs(40) {
                    warn!("No ping received in the last 40 seconds, reconnecting...");
                    break;
                }
            }
        };

        tokio::select! {
            _ = check_has_ping_loop => {},
            _ = handle_stream => {}
        }
        Ok(())
    }

    async fn handle_server_message(&self, message: ServerMessage) -> Result<()> {
        match message.message_type.as_str() {
            "offer" => {
                let data = &message.data;
                let uuid = data["uuid"].as_str().unwrap_or("unknown");
                let offer = data["offer"].as_str().unwrap_or("");
                let client_ip = data["client_ip"].as_str().unwrap_or("unknown");

                info!("Received offer from client {} (IP: {})", uuid, client_ip);
                let answer = self.handle_offer(uuid, offer, client_ip).await?;

                // Send answer back via HTTP POST
                let answer_msg = AnswerMessage { answer };
                let url = format!("{}/rport/answer/{}", self.server_url, uuid);

                let response = self.client.post(&url).json(&answer_msg).send().await?;
                if response.status().is_success() {
                    info!("Answer sent successfully");
                } else {
                    error!("Failed to send answer: {}", response.status());
                }
            }
            "ping" => {
                debug!("Received ping from server");
            }
            _ => {
                warn!("Unknown message type: {}", message.message_type);
            }
        }
        Ok(())
    }

    async fn handle_offer(
        &self,
        session_id: &str,
        offer_sdp: &str,
        client_ip: &str,
    ) -> Result<String> {
        info!(
            "Creating WebRTC peer connection for session: {}",
            session_id
        );

        let peer_connection = self.create_peer_connection().await?;

        // Strip IPv6 candidates from incoming offer
        let filtered_offer_sdp = strip_ipv6_candidates(offer_sdp);

        // Set remote description first
        let offer = RTCSessionDescription::offer(filtered_offer_sdp)?;
        peer_connection.set_remote_description(offer).await?;

        // Set up data channel handler
        let target_host = self.target_host.clone();
        let target_port = self.target_port;
        let client_ip = client_ip.to_string();

        peer_connection.on_data_channel(Box::new(move |data_channel| {
            let target_host = target_host.clone();
            let target_port = target_port;
            let data_channel_clone = data_channel.clone();
            let client_ip_clone = client_ip.clone();
            let cancel_token = tokio_util::sync::CancellationToken::new();
            let (tcp_write_tx, tcp_write_rx) = mpsc::unbounded_channel();

            data_channel.on_message(Box::new(move |msg| {
                let tcp_write_tx_clone = tcp_write_tx.clone();
                Box::pin(async move {
                    tcp_write_tx_clone.send(msg.data).ok();
                })
            }));

            data_channel.on_close(Box::new({
                let cancel_token_ref = cancel_token.clone();
                move || {
                    let cancel_token = cancel_token_ref.clone();
                    Box::pin(async move {
                        info!("Data channel closed, cancelling forwarding");
                        cancel_token.cancel();
                    })
                }
            }));

            data_channel.on_open(Box::new(move || {
                let target_host = target_host.clone();
                let target_port = target_port;
                let data_channel = Arc::clone(&data_channel_clone);
                let client_ip_clone = client_ip_clone.clone();
                Box::pin(async move {
                    // Start TCP forwarding
                    tokio::spawn(async move {
                        tcp_webrtc_forwarding(
                            cancel_token,
                            tcp_write_rx,
                            client_ip_clone,
                            data_channel,
                            &target_host,
                            target_port,
                        )
                        .await
                        .ok();
                    });
                })
            }));
            Box::pin(async move {})
        }));

        // Create answer
        let answer = peer_connection.create_answer(None).await?;
        peer_connection
            .set_local_description(answer.clone())
            .await?;

        // Wait for ICE gathering to complete
        peer_connection
            .gathering_complete_promise()
            .await
            .recv()
            .await;

        let answer_sdp = peer_connection
            .local_description()
            .await
            .ok_or_else(|| anyhow!("Failed to get local description"))?
            .sdp;

        // Strip IPv6 candidates from answer
        let filtered_answer_sdp = strip_ipv6_candidates(&answer_sdp);

        Ok(filtered_answer_sdp)
    }

    async fn create_peer_connection(&self) -> Result<Arc<RTCPeerConnection>> {
        self.webrtc_config.create_peer_connection().await
    }
}

async fn tcp_webrtc_forwarding(
    cancel_token: tokio_util::sync::CancellationToken,
    mut tcp_write_rx: mpsc::UnboundedReceiver<Bytes>,
    client_ip: String,
    data_channel: Arc<RTCDataChannel>,
    target_host: &str,
    target_port: u16,
) -> Result<()> {
    let tcp_stream = match TcpStream::connect(format!("{}:{}", target_host, target_port)).await {
        Ok(stream) => stream,
        Err(e) => {
            error!(
                "Failed to connect to {}: {}: {}",
                target_host, target_port, e
            );
            return Err(anyhow!(
                "Failed to connect to {}: {}",
                target_host,
                target_port
            ));
        }
    };

    info!(
        client_ip,
        "Setting up bidirectional forwarding for {}:{}", target_host, target_port
    );

    let (mut tcp_read, mut tcp_write) = tcp_stream.into_split();
    let recv_from_tcp = async {
        let mut buffer = [0u8; 4096];
        loop {
            match tcp_read.read(&mut buffer).await {
                Ok(0) => {
                    info!("TCP connection closed");
                    break;
                }
                Ok(n) => {
                    let data = Bytes::from(buffer[..n].to_vec());
                    if let Err(e) = data_channel.send(&data).await {
                        error!("Failed to send data through WebRTC: {}", e);
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    };
    let recv_from_data_channel = async {
        while let Some(msg) = tcp_write_rx.recv().await {
            if tcp_write.write_all(&msg).await.is_err() {
                error!("Failed to write to TCP stream");
                break;
            }
        }
    };

    tokio::select! {
        _ = cancel_token.cancelled() => {
            info!(client_ip, "Cancellation requested");
        }
        _ = recv_from_data_channel => {
            info!(client_ip, "Data channel closed");
        }
        _ = recv_from_tcp => {
            info!(client_ip, "TCP stream closed");
        }
    }
    Ok(())
}
