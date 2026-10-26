use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::{
    path::Path,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, PartialEq)]
pub struct SentMessage {
    pub id: i64,
    pub text: String,
    pub sent_at: i64,
}
pub struct History {
    db: Connection,
    pub entries: Arc<Vec<SentMessage>>,
}
impl History {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let db = Connection::open(path).context("Could not open message history")?;
        db.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS sent_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT, text TEXT NOT NULL, sent_at INTEGER NOT NULL
            );",
        )?;
        let entries = {
            let mut query =
                db.prepare("SELECT id, text, sent_at FROM sent_messages ORDER BY id")?;
            query
                .query_map([], |r| {
                    Ok(SentMessage {
                        id: r.get(0)?,
                        text: r.get(1)?,
                        sent_at: r.get(2)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        Ok(Self {
            db,
            entries: Arc::new(entries),
        })
    }
    pub fn record(&mut self, text: &str) -> Result<()> {
        let sent_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        self.db.execute(
            "INSERT INTO sent_messages(text, sent_at) VALUES (?1, ?2)",
            params![text, sent_at],
        )?;
        Arc::make_mut(&mut self.entries).push(SentMessage {
            id: self.db.last_insert_rowid(),
            text: text.into(),
            sent_at,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn survives_reopen_preserves_unicode_multiline_and_duplicates() {
        let path = std::env::temp_dir().join(format!(
            "speak-history-{}-{}.sqlite",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut store = History::open(&path).unwrap();
        let text = "NASA?\n你好！ \"woof\" <script> & emojis 🐾";
        store.record(text).unwrap();
        let old_snapshot = store.entries.clone();
        store.record(text).unwrap();
        assert_eq!(old_snapshot.len(), 1);
        drop(store);
        let store = History::open(&path).unwrap();
        assert_eq!(store.entries.len(), 2);
        assert_eq!(store.entries[0].text, text);
        assert!(store.entries[1].id > store.entries[0].id);
        drop(store);
        std::fs::remove_file(path).unwrap();
    }
}
