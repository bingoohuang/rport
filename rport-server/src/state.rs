use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub id: String,
    pub token: String,
    pub last_ping: SystemTime,
}
