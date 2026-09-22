use super::Store;
use crate::{domain::now, domain::replay::ReplayJob};
use rusqlite::params;
impl Store {
    pub fn forget_replay(&self, id: &str) -> Result<(), String> {
        self.0
            .execute("DELETE FROM replay_files WHERE game_id=?1", [id])
            .map_err(|_| "library.storageError")?;
        Ok(())
    }
    pub fn replay_files(&self) -> Result<Vec<ReplayJob>, String> {
        let mut statement = self
            .0
            .prepare("SELECT game_id,bytes,version FROM replay_files ORDER BY downloaded_at DESC")
            .map_err(|_| "library.storageError")?;
        let rows = statement
            .query_map([], |r| {
                let bytes = r.get::<_, i64>(1)? as f64;
                Ok(ReplayJob {
                    id: r.get(0)?,
                    state: "ready".into(),
                    received: bytes,
                    total: Some(bytes),
                    version: Some(r.get(2)?),
                    error: None,
                })
            })
            .map_err(|_| "library.storageError")?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| "library.storageError".into())
    }
    pub fn save_replay(&self, id: &str, bytes: u64, version: &str) -> Result<(), String> {
        self.0.execute("INSERT INTO replay_files(game_id,bytes,version,downloaded_at) VALUES (?1,?2,?3,?4) ON CONFLICT(game_id) DO UPDATE SET bytes=excluded.bytes,version=excluded.version,downloaded_at=excluded.downloaded_at",params![id,i64::try_from(bytes).map_err(|_| "replay.tooLarge")?,version,now()]).map_err(|_| "library.storageError")?;
        Ok(())
    }
}
