use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::RwLock;
use time::OffsetDateTime;

use crate::error::AuthError;
use crate::types::{SessionId, SessionInfo, SessionStore};

pub struct MemorySessionStore {
    sessions: RwLock<HashMap<SessionId, SessionInfo>>,
}

impl MemorySessionStore {
    pub fn new() -> Self {
        MemorySessionStore {
            sessions: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl SessionStore for MemorySessionStore {
    async fn save(&self, id: &SessionId, info: &SessionInfo) -> Result<(), AuthError> {
        self.sessions.write().await.insert(id.clone(), info.clone());
        Ok(())
    }

    async fn load(&self, id: &SessionId) -> Result<Option<SessionInfo>, AuthError> {
        Ok(self.sessions.read().await.get(id).cloned())
    }

    async fn delete(&self, id: &SessionId) -> Result<(), AuthError> {
        self.sessions.write().await.remove(id);
        Ok(())
    }

    async fn cleanup(&self) -> Result<(), AuthError> {
        let now = OffsetDateTime::now_utc();
        self.sessions.write().await.retain(|_, info| info.expires_at > now);
        Ok(())
    }
}
