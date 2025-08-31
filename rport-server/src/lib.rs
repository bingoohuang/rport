use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use uuid::Uuid;
pub mod clientaddr;
pub mod handler;
pub mod state;
pub mod turn_server;
pub use state::*;
pub use turn_server::*;

use crate::handler::{AgentConnection, PendingOffer};

#[derive(Clone)]
pub struct AppState {
    pub agents: Arc<RwLock<HashMap<String, AgentConnection>>>,
    pub pending_offers: Arc<RwLock<HashMap<Uuid, PendingOffer>>>,
    pub turn_server: Arc<TurnServer>,
}

impl AppState {
    pub fn new_with_turn(turn_server: Arc<TurnServer>) -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            pending_offers: Arc::new(RwLock::new(HashMap::new())),
            turn_server,
        }
    }
}
