use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, params};
use std::{
    path::Path,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageKind {
    Speak,
    Write,
}
impl MessageKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Speak => "Speak",
            Self::Write => "Write",
        }
    }
    fn value(self) -> &'static str {
        match self {
            Self::Speak => "speak",
            Self::Write => "write",
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy: f64,
    pub captured_at: f64,
}
impl Location {
    pub fn valid_for(&self, sent_at: i64) -> bool {
        self.latitude.is_finite()
            && self.longitude.is_finite()
            && self.accuracy.is_finite()
            && (-90.0..=90.0).contains(&self.latitude)
            && (-180.0..=180.0).contains(&self.longitude)
            && self.accuracy >= 0.0
            && self.captured_at.is_finite()
            && (self.captured_at - sent_at as f64).abs() <= 60.0
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct SentMessage {
    pub id: i64,
    pub text: String,
    pub sent_at: i64,
    pub kind: MessageKind,
    pub location: Option<Location>,
    pub location_pending: bool,
    pub gap: f64,
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
        let mut db = Connection::open(path).context("Could not open message history")?;
        db.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS sent_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT, text TEXT NOT NULL, sent_at INTEGER NOT NULL
            );",
        )?;
        let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version < 1 {
            let tx = db.transaction()?;
            tx.execute_batch(
                "ALTER TABLE sent_messages ADD COLUMN kind TEXT NOT NULL DEFAULT 'speak';
                ALTER TABLE sent_messages ADD COLUMN latitude REAL;
                ALTER TABLE sent_messages ADD COLUMN longitude REAL;
                ALTER TABLE sent_messages ADD COLUMN accuracy REAL;
                ALTER TABLE sent_messages ADD COLUMN captured_at REAL;
                ALTER TABLE sent_messages ADD COLUMN location_pending INTEGER NOT NULL DEFAULT 0;
                ALTER TABLE sent_messages ADD COLUMN gap REAL NOT NULL DEFAULT 0;
                PRAGMA user_version=1;",
            )?;
            tx.commit()?;
        }
        // An interrupted location request cannot be recovered on a later launch.
        db.execute(
            "UPDATE sent_messages SET location_pending=0 WHERE location_pending<>0",
            [],
        )?;
        let entries = {
            let mut query = db.prepare("SELECT id,text,sent_at,kind,latitude,longitude,accuracy,captured_at,location_pending,gap FROM sent_messages ORDER BY id")?;
            query
                .query_map([], |r| {
                    let lat: Option<f64> = r.get(4)?;
                    Ok(SentMessage {
                        id: r.get(0)?,
                        text: r.get(1)?,
                        sent_at: r.get(2)?,
                        kind: if r.get::<_, String>(3)? == "write" {
                            MessageKind::Write
                        } else {
                            MessageKind::Speak
                        },
                        location: match lat {
                            Some(latitude) => Some(Location {
                                latitude,
                                longitude: r.get(5)?,
                                accuracy: r.get(6)?,
                                captured_at: r.get(7)?,
                            }),
                            None => None,
                        },
                        location_pending: r.get(8)?,
                        gap: r.get(9)?,
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
        self.record_kind(text, MessageKind::Speak).map(|_| ())
    }
    pub fn record_kind(&mut self, text: &str, kind: MessageKind) -> Result<i64> {
        let sent_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        self.db.execute(
            "INSERT INTO sent_messages(text,sent_at,kind,location_pending) VALUES (?1,?2,?3,1)",
            params![text, sent_at, kind.value()],
        )?;
        let id = self.db.last_insert_rowid();
        Arc::make_mut(&mut self.entries).push(SentMessage {
            id,
            text: text.into(),
            sent_at,
            kind,
            location: None,
            location_pending: true,
            gap: 0.0,
        });
        Ok(id)
    }
    pub fn set_location(&mut self, id: i64, location: Option<Location>) -> Result<()> {
        let Some(index) = self.entries.iter().position(|e| e.id == id) else {
            return Ok(());
        };
        let location = location.filter(|l| l.valid_for(self.entries[index].sent_at));
        self.db.execute("UPDATE sent_messages SET latitude=?1,longitude=?2,accuracy=?3,captured_at=?4,location_pending=0 WHERE id=?5",
            params![location.as_ref().map(|l|l.latitude), location.as_ref().map(|l|l.longitude), location.as_ref().map(|l|l.accuracy), location.as_ref().map(|l|l.captured_at),id])?;
        let entry = &mut Arc::make_mut(&mut self.entries)[index];
        entry.location = location;
        entry.location_pending = false;
        Ok(())
    }
    pub fn set_gap(&mut self, id: i64, gap: f64) -> Result<()> {
        ensure!(
            gap.is_finite() && (0.0..=100_000.0).contains(&gap),
            "Invalid spacing"
        );
        let Some(index) = self
            .entries
            .iter()
            .position(|e| e.id == id && e.kind == MessageKind::Write)
        else {
            return Ok(());
        };
        self.db.execute(
            "UPDATE sent_messages SET gap=?1 WHERE id=?2",
            params![gap, id],
        )?;
        Arc::make_mut(&mut self.entries)[index].gap = gap;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migrates_existing_history_and_preserves_write_spacing_and_location() {
        let path = std::env::temp_dir().join(format!(
            "speak-history-{}-{}.sqlite",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let db = Connection::open(&path).unwrap();
        db.execute_batch("CREATE TABLE sent_messages(id INTEGER PRIMARY KEY AUTOINCREMENT,text TEXT NOT NULL,sent_at INTEGER NOT NULL); INSERT INTO sent_messages(text,sent_at) VALUES ('Old message',1234);").unwrap();
        drop(db);
        let mut store = History::open(&path).unwrap();
        assert_eq!(store.entries[0].sent_at, 1234);
        assert_eq!(store.entries[0].location, None);
        let text = "NASA?\n你好！ 🐾";
        let id = store.record_kind(text, MessageKind::Write).unwrap();
        let sent_at = store.entries[1].sent_at;
        let old = store.entries.clone();
        store.set_gap(id, 800.0).unwrap();
        store
            .set_location(
                id,
                Some(Location {
                    latitude: 37.0,
                    longitude: -122.0,
                    accuracy: 100.0,
                    captured_at: sent_at as f64,
                }),
            )
            .unwrap();
        assert_eq!(old[1].gap, 0.0);
        assert!(store.set_gap(id, f64::NAN).is_err());
        store.record(text).unwrap();
        drop(store);
        let mut store = History::open(&path).unwrap();
        assert_eq!(store.entries.len(), 3);
        assert_eq!(store.entries[1].text, text);
        assert_eq!(store.entries[1].kind, MessageKind::Write);
        assert_eq!(store.entries[1].gap, 800.0);
        assert!(store.entries[1].location.is_some());
        assert!(!store.entries[2].location_pending);
        store
            .set_location(
                id,
                Some(Location {
                    latitude: 37.0,
                    longitude: -122.0,
                    accuracy: 100.0,
                    captured_at: sent_at as f64 - 3600.0,
                }),
            )
            .unwrap();
        assert!(store.entries[1].location.is_none());
        drop(store);
        std::fs::remove_file(path).unwrap();
    }
}
