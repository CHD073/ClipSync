use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Clone, Debug)]
pub struct WsMessage {
    pub data: String,
    pub source_device_id: Option<String>,
    pub target_device_id: Option<String>,
}

pub type WsBroadcast = Arc<broadcast::Sender<WsMessage>>;

pub fn new_broadcast() -> WsBroadcast {
    Arc::new(broadcast::channel::<WsMessage>(256).0)
}
