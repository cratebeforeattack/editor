use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const EDITOR_PROTOCOL_VERSION: u16 = 1;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum DisconnectReason {
    ClientToOld,
    ClientToNew,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum EditorServerMessage {
    Welcome {},
    ConnectionAborted {
        reason: DisconnectReason,
    },
    JoinedSession {
        id: u64,
        url: String,
        new_session: bool,
    },
    LeftSession {
        id: u64,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum EditorClientMessage {
    Introduction {
        protocol_version: u16,
        build: String,
    },
    Upload {
        map_hash: u64,
        content: Arc<Blob>,
    },
}

// solely a wrapper for debug formatting
#[derive(Clone, Serialize, Deserialize, PartialEq, Hash)]
pub struct Blob(pub Vec<u8>);
impl std::fmt::Debug for Blob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let len = self.0.len();
        f.debug_struct("Blob").field("len", &len).finish()
    }
}
