use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectMessage {
    pub token: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferMessage {
    pub id: String,
    pub offer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferResponse {
    pub uuid: Uuid,
    pub offer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnswerMessage {
    pub answer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerMessage {
    pub message_type: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: String,
    pub token: String,
    pub last_ping: std::time::SystemTime,
}

pub const PING_INTERVAL: u64 = 30; // seconds
pub const RECONNECT_INTERVAL: u64 = 5; // seconds
