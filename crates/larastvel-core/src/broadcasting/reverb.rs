//! # Reverb Database Scaling Driver
//!
//! Laravel 13's Reverb "database" driver removes the Redis requirement for
//! horizontal WebSocket scaling: multiple Reverb instances coordinate
//! pub/sub through the application database instead of a message broker.
//!
//! This module mirrors that design for the native broadcaster:
//!
//! * [`ReverbScalingStore`] — a `reverb_scaling` table holding pending
//!   broadcast messages (channel + payload) that any instance can read.
//! * [`ReverbDatabaseBroadcaster`] — a [`super::Broadcaster`] that delivers
//!   messages to its local subscribers immediately AND publishes them to the
//!   store, so other instances (running their own poll loop) fan them out to
//!   their own clients.
//! * [`ReverbDatabaseBroadcaster::spawn_scaling_poller`] — a background task
//!   that polls the store and replays new messages to local subscribers.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};

use super::{
    BroadcastError, BroadcastMessage, Broadcaster, ChannelAuthCallback, PresenceChannelData,
    SubscriberRegistry,
};

/// A broadcast message row read from the scaling store.
#[derive(Debug, Clone)]
pub struct BroadcastRow {
    pub id: i64,
    pub channel: String,
    pub message: String,
}

/// SQLite-backed pub/sub bus shared by broadcaster instances.
#[derive(Debug, Clone)]
pub struct ReverbScalingStore {
    db: DatabaseConnection,
    table_name: String,
}

impl ReverbScalingStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            table_name: "reverb_scaling".to_string(),
        }
    }

    pub fn with_table(mut self, table: &str) -> Self {
        self.table_name = table.to_string();
        self
    }

    pub fn table_name(&self) -> &str {
        &self.table_name
    }

    pub async fn ensure_table_exists(&self) -> Result<(), BroadcastError> {
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {} (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel TEXT NOT NULL,
                message TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                sent_at INTEGER
            )",
            self.table_name
        );
        self.db
            .execute(Statement::from_string(DatabaseBackend::Sqlite, sql))
            .await
            .map_err(|e| BroadcastError::Failed(format!("Failed to create reverb table: {}", e)))?;
        Ok(())
    }

    /// Publish a message and return its row id.
    pub async fn publish(&self, channel: &str, message: &str) -> Result<i64, BroadcastError> {
        let now = unix_now();
        let sql = format!(
            "INSERT INTO {} (channel, message, created_at) VALUES (?1, ?2, ?3)",
            self.table_name
        );
        let result = self
            .db
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                [channel.into(), message.into(), now.into()],
            ))
            .await
            .map_err(|e| BroadcastError::Failed(format!("Failed to publish message: {}", e)))?;
        Ok(result.last_insert_id().try_into().unwrap_or(0))
    }

    /// Fetch rows with `id > last_id` in insertion order.
    pub async fn poll_after(
        &self,
        last_id: i64,
        limit: u64,
    ) -> Result<Vec<BroadcastRow>, BroadcastError> {
        let sql = format!(
            "SELECT id, channel, message FROM {} WHERE id > ?1 ORDER BY id ASC LIMIT ?2",
            self.table_name
        );
        let result = self
            .db
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                [last_id.into(), (limit as i64).into()],
            ))
            .await
            .map_err(|e| BroadcastError::Failed(format!("Failed to poll reverb table: {}", e)))?;

        result
            .into_iter()
            .map(|row| {
                Ok(BroadcastRow {
                    id: row
                        .try_get_by_index::<i64>(0)
                        .map_err(|e| BroadcastError::Failed(format!("Bad row id: {}", e)))?,
                    channel: row
                        .try_get_by_index::<String>(1)
                        .map_err(|e| BroadcastError::Failed(format!("Bad row channel: {}", e)))?,
                    message: row
                        .try_get_by_index::<String>(2)
                        .map_err(|e| BroadcastError::Failed(format!("Bad row message: {}", e)))?,
                })
            })
            .collect()
    }

    /// Number of rows not yet marked as sent.
    pub async fn pending_count(&self) -> usize {
        let sql = format!(
            "SELECT COUNT(*) as cnt FROM {} WHERE sent_at IS NULL",
            self.table_name
        );
        let result = self
            .db
            .query_one(Statement::from_string(DatabaseBackend::Sqlite, sql))
            .await;
        match result {
            Ok(Some(row)) => row.try_get_by_index::<i64>(0).ok().unwrap_or(0) as usize,
            _ => 0,
        }
    }

    /// Mark rows as consumed by a subscriber instance.
    pub async fn mark_sent(&self, ids: &[i64]) -> Result<(), BroadcastError> {
        if ids.is_empty() {
            return Ok(());
        }
        let placeholders: Vec<String> = (2..=ids.len() + 1).map(|i| format!("?{}", i)).collect();
        let sql = format!(
            "UPDATE {} SET sent_at = ?1 WHERE id IN ({})",
            self.table_name,
            placeholders.join(", ")
        );
        let mut values = vec![unix_now().into()];
        values.extend(ids.iter().map(|id| (*id).into()));
        self.db
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                values,
            ))
            .await
            .map_err(|e| BroadcastError::Failed(format!("Failed to mark messages sent: {}", e)))?;
        Ok(())
    }
}

/// A native broadcaster that coordinates across instances through the
/// database (Laravel 13's Reverb `scaling.driver = database`).
#[derive(Debug)]
pub struct ReverbDatabaseBroadcaster {
    name: String,
    registry: SubscriberRegistry,
    store: ReverbScalingStore,
    instance_id: String,
    last_id: Arc<AtomicI64>,
}

impl ReverbDatabaseBroadcaster {
    pub fn new(name: &str, registry: SubscriberRegistry, store: ReverbScalingStore) -> Self {
        Self {
            name: name.to_string(),
            registry,
            store,
            instance_id: uuid::Uuid::new_v4().to_string(),
            last_id: Arc::new(AtomicI64::new(0)),
        }
    }

    /// Override the instance id (useful for stable multi-instance setups).
    pub fn with_instance_id(mut self, instance_id: &str) -> Self {
        self.instance_id = instance_id.to_string();
        self
    }

    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    /// The store shared with other instances.
    pub fn store(&self) -> &ReverbScalingStore {
        &self.store
    }

    /// The id of the last message consumed by this instance.
    pub fn last_message_id(&self) -> i64 {
        self.last_id.load(Ordering::SeqCst)
    }

    pub async fn ensure_table_exists(&self) -> Result<(), BroadcastError> {
        self.store.ensure_table_exists().await
    }

    /// Poll the store once and replay new messages to local subscribers.
    /// Returns the number of messages replayed.
    pub async fn drain_pending(&self) -> Result<usize, BroadcastError> {
        let rows = self
            .store
            .poll_after(self.last_id.load(Ordering::SeqCst), 500)
            .await?;
        let ids: Vec<i64> = rows.iter().map(|row| row.id).collect();
        for row in &rows {
            self.registry
                .broadcast_to_channels(std::slice::from_ref(&row.channel), &row.message)
                .await;
            self.last_id.store(row.id, Ordering::SeqCst);
        }
        self.store.mark_sent(&ids).await?;
        Ok(rows.len())
    }

    /// Spawn a background task that polls the store every `interval` and
    /// replays new messages to local subscribers. Stop it by aborting the
    /// returned handle.
    pub fn spawn_scaling_poller(
        self: &Arc<Self>,
        interval: std::time::Duration,
    ) -> tokio::task::JoinHandle<()> {
        let broadcaster = self.clone();
        tokio::spawn(async move {
            loop {
                if let Err(e) = broadcaster.drain_pending().await {
                    tracing::warn!(
                        "[ReverbDatabaseBroadcaster] {} scaling poll failed: {}",
                        broadcaster.name,
                        e
                    );
                }
                tokio::time::sleep(interval).await;
            }
        })
    }

    /// Same as [`Self::spawn_scaling_poller`] but inline for embedding in a
    /// longer-running task.
    pub async fn run_scaling_poller(self: Arc<Self>, interval: std::time::Duration) {
        loop {
            let _ = self.drain_pending().await;
            tokio::time::sleep(interval).await;
        }
    }
}

impl Clone for ReverbDatabaseBroadcaster {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            registry: self.registry.clone(),
            store: self.store.clone(),
            instance_id: self.instance_id.clone(),
            last_id: self.last_id.clone(),
        }
    }
}

#[async_trait]
impl Broadcaster for ReverbDatabaseBroadcaster {
    async fn broadcast(&self, message: BroadcastMessage) -> Result<(), BroadcastError> {
        let channels = message.channels.clone();
        let payload = serde_json::json!({
            "event": message.event,
            "data": message.data,
            "channel": channels.first().map(|s| s.as_str()),
        });
        let text = serde_json::to_string(&payload)
            .map_err(|e| BroadcastError::Failed(format!("Serialization error: {}", e)))?;

        // Deliver locally first, then publish for other instances.
        self.registry.broadcast_to_channels(&channels, &text).await;
        let channel = channels
            .first()
            .cloned()
            .unwrap_or_else(|| "default".to_string());
        self.store.publish(&channel, &text).await?;
        Ok(())
    }

    async fn authenticate(
        &self,
        _channel_name: &str,
        _socket_id: &str,
        _callback: Option<&ChannelAuthCallback>,
    ) -> Result<String, BroadcastError> {
        let response = serde_json::json!({
            "auth": format!("{}:reverb-db", self.name),
        });
        serde_json::to_string(&response)
            .map_err(|e| BroadcastError::AuthError(format!("Serialization error: {}", e)))
    }

    async fn authenticate_presence(
        &self,
        _channel_name: &str,
        _socket_id: &str,
        channel_data: &PresenceChannelData,
        _callback: Option<&ChannelAuthCallback>,
    ) -> Result<String, BroadcastError> {
        let channel_data_json = serde_json::to_string(channel_data)
            .map_err(|e| BroadcastError::AuthError(format!("Serialization error: {}", e)))?;
        let response = serde_json::json!({
            "auth": format!("{}:reverb-db", self.name),
            "channel_data": channel_data_json,
        });
        serde_json::to_string(&response)
            .map_err(|e| BroadcastError::AuthError(format!("Serialization error: {}", e)))
    }

    fn name(&self) -> &str {
        &self.name
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    async fn setup_db() -> DatabaseConnection {
        sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory SQLite")
    }

    fn message(event: &str, channel: &str) -> BroadcastMessage {
        BroadcastMessage::new(
            event,
            serde_json::json!({"key": "value"}),
            vec![channel.to_string()],
        )
    }

    #[tokio::test]
    async fn test_store_publish_and_poll() {
        let store = ReverbScalingStore::new(setup_db().await);
        store.ensure_table_exists().await.unwrap();

        let id = store.publish("chat", "hello").await.unwrap();
        assert!(id > 0);

        let rows = store.poll_after(0, 10).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].channel, "chat");
        assert_eq!(rows[0].message, "hello");
        assert_eq!(store.pending_count().await, 1);
    }

    #[tokio::test]
    async fn test_store_poll_incremental() {
        let store = ReverbScalingStore::new(setup_db().await);
        store.ensure_table_exists().await.unwrap();

        let id1 = store.publish("a", "m1").await.unwrap();
        let _ = store.publish("a", "m2").await.unwrap();

        let rows = store.poll_after(id1, 10).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].message, "m2");
    }

    #[tokio::test]
    async fn test_store_mark_sent() {
        let store = ReverbScalingStore::new(setup_db().await);
        store.ensure_table_exists().await.unwrap();

        let id1 = store.publish("a", "m1").await.unwrap();
        let id2 = store.publish("a", "m2").await.unwrap();
        store.mark_sent(&[id1, id2]).await.unwrap();
        assert_eq!(store.pending_count().await, 0);
    }

    #[tokio::test]
    async fn test_store_mark_sent_empty_is_noop() {
        let store = ReverbScalingStore::new(setup_db().await);
        store.ensure_table_exists().await.unwrap();
        store.mark_sent(&[]).await.unwrap();
        assert_eq!(store.pending_count().await, 0);
    }

    #[tokio::test]
    async fn test_broadcaster_delivers_locally_and_persists() {
        let db = setup_db().await;
        let store = ReverbScalingStore::new(db.clone());
        store.ensure_table_exists().await.unwrap();

        let registry = SubscriberRegistry::new();
        let broadcaster = ReverbDatabaseBroadcaster::new("native", registry.clone(), store);

        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        registry.subscribe("chat", tx).await;

        broadcaster
            .broadcast(message("test-event", "chat"))
            .await
            .unwrap();

        let received = rx.recv().await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&received).unwrap();
        assert_eq!(parsed["event"], "test-event");

        assert_eq!(broadcaster.store().pending_count().await, 1);
    }

    #[tokio::test]
    async fn test_cross_instance_fanout_via_store() {
        let db = setup_db().await;
        let store = ReverbScalingStore::new(db.clone());
        store.ensure_table_exists().await.unwrap();

        // Instance A publishes; instance B (different registry) picks the
        // message up from the database and replays it to its own clients.
        let registry_a = SubscriberRegistry::new();
        let registry_b = SubscriberRegistry::new();
        let broadcaster_a = ReverbDatabaseBroadcaster::new("a", registry_a, store.clone());
        let broadcaster_b = ReverbDatabaseBroadcaster::new("b", registry_b.clone(), store);

        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        registry_b.subscribe("chat", tx).await;

        broadcaster_a
            .broadcast(message("cross-instance", "chat"))
            .await
            .unwrap();

        assert_eq!(broadcaster_b.drain_pending().await.unwrap(), 1);
        let received = rx.recv().await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&received).unwrap();
        assert_eq!(parsed["event"], "cross-instance");
        assert_eq!(broadcaster_b.last_message_id(), 1);
        assert_eq!(broadcaster_b.store().pending_count().await, 0);
    }

    #[tokio::test]
    async fn test_scaling_poller_replays_async() {
        let db = setup_db().await;
        let store = ReverbScalingStore::new(db.clone());
        store.ensure_table_exists().await.unwrap();

        let registry_a = SubscriberRegistry::new();
        let registry_b = SubscriberRegistry::new();
        let broadcaster_a = Arc::new(ReverbDatabaseBroadcaster::new(
            "a",
            registry_a,
            store.clone(),
        ));
        let broadcaster_b = Arc::new(ReverbDatabaseBroadcaster::new(
            "b",
            registry_b.clone(),
            store,
        ));

        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        registry_b.subscribe("alerts", tx).await;

        let poller = broadcaster_b.spawn_scaling_poller(std::time::Duration::from_millis(20));

        broadcaster_a
            .broadcast(message("async-event", "alerts"))
            .await
            .unwrap();

        let received = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("timed out waiting for async fanout")
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&received).unwrap();
        assert_eq!(parsed["event"], "async-event");

        poller.abort();
    }

    #[tokio::test]
    async fn test_broadcaster_authenticate_and_name() {
        let store = ReverbScalingStore::new(setup_db().await);
        let registry = SubscriberRegistry::new();
        let broadcaster = ReverbDatabaseBroadcaster::new("reverb-db", registry, store);

        let auth = broadcaster
            .authenticate("private-x", "1.2", None)
            .await
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&auth).unwrap();
        assert!(parsed["auth"].is_string());

        let presence = broadcaster
            .authenticate_presence(
                "presence-x",
                "1.2",
                &PresenceChannelData {
                    user_id: "u1".to_string(),
                    user_info: None,
                },
                None,
            )
            .await
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&presence).unwrap();
        assert!(parsed["channel_data"].is_string());

        assert_eq!(broadcaster.name(), "reverb-db");
        assert!(!broadcaster.instance_id().is_empty());
    }

    #[tokio::test]
    async fn test_store_custom_table_name() {
        let store = ReverbScalingStore::new(setup_db().await).with_table("scaling");
        assert_eq!(store.table_name(), "scaling");
        store.ensure_table_exists().await.unwrap();
        store.publish("chat", "x").await.unwrap();
        assert_eq!(store.pending_count().await, 1);
    }
}
