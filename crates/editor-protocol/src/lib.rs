use serde::{Deserialize, Serialize};
use std::sync::Arc;

const EDITOR_PROTOCOL_VERSION: u16 = 1;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum DisconnectReason {
    ClientToOld,
    ClientToNew,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum EditorServerMessage {
    Welcome {},
    ConnectionAborted { reason: DisconnectReason },
    JoinedSession { id: u64 },
    LeftSession { id: u64 },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum EditorClientMessage {
    Introduction {
        protocol_version: u16,
        build: String,
    },
    Upload {
        map_hash: u64,
        content: Arc<Vec<u8>>,
    },
}
