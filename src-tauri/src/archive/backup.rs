use super::*;
mod merge;
pub mod stream;
mod validation;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    hash::{Hash, Hasher},
    io::Write,
    path::Path,
};
use validation::validate_store;

const TABLES: &[&str] = &[
    "accounts",
    "games",
    "sessions",
    "segments",
    "participations",
    "user_state",
    "review_games",
    "replay_files",
    "subscriptions",
    "followed_games",
];
#[derive(Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/library/")]
pub struct BackupPreview {
    created_at: f64,
    pub fingerprint: String,
    counts: BTreeMap<String, u32>,
}
fn invalid(_: impl std::fmt::Display) -> String {
    "backup.invalid".into()
}

impl Store {
    pub fn backup_fingerprint(&self) -> Result<String> {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        for table in TABLES {
            table.hash(&mut hash);
            stream::rows(&self.0, table, |row| {
                serde_json::to_vec(&row).map_err(invalid)?.hash(&mut hash);
                Ok(())
            })?;
        }
        Ok(format!("{:016x}", hash.finish()))
    }
    pub fn backup_preview(&self, source: &Store, created_at: i64) -> Result<BackupPreview> {
        let mut counts = BTreeMap::new();
        for table in TABLES {
            let count: u32 = source
                .0
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
                .map_err(sql)?;
            counts.insert((*table).into(), count);
        }
        let ui: String = source
            .0
            .query_row("SELECT data FROM user_state WHERE id=1", [], |r| r.get(0))
            .map_err(sql)?;
        let ui: LiveUiState = serde_json::from_str(&ui).map_err(invalid)?;
        counts.insert("notes".into(), ui.notes.len() as u32);
        counts.insert("players".into(), ui.players.len() as u32);
        Ok(BackupPreview {
            created_at: created_at as f64,
            fingerprint: self.backup_fingerprint()?,
            counts,
        })
    }
    pub fn restore_snapshot(
        &mut self,
        source: &Store,
        fingerprint: &str,
        directory: &Path,
    ) -> Result<String> {
        if self.backup_fingerprint()? != fingerprint {
            return Err("backup.changed".into());
        }
        let revision: u32 = self
            .0
            .query_row("SELECT revision FROM user_state WHERE id=1", [], |r| {
                r.get(0)
            })
            .map_err(sql)?;
        let revision = revision.checked_add(1).ok_or("backup.invalid")?;
        // The current database is copied durably before the replacement transaction.
        std::fs::create_dir_all(directory).map_err(|_| "backup.writeFailed")?;
        let path = directory.join(format!("before-restore-{}-{}.jsonl", now(), id()));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| "backup.writeFailed")?;
        if let Err(error) = self
            .write_backup(&mut file)
            .and_then(|_| file.sync_all().map_err(|_| "backup.writeFailed".into()))
        {
            drop(file);
            let _ = std::fs::remove_file(&path);
            return Err(error);
        }
        let tx = self.0.transaction().map_err(sql)?;
        stream::copy(&source.0, &tx)?;
        tx.execute("UPDATE user_state SET revision=?1 WHERE id=1", [revision])
            .map_err(sql)?;
        tx.execute("UPDATE sessions SET ended_at=last_activity,end_reason='restored' WHERE ended_at IS NULL", []).map_err(sql)?;
        tx.execute(
            "UPDATE participations SET state='pending' WHERE state='playing'",
            [],
        )
        .map_err(sql)?;
        tx.execute(
            "UPDATE subscriptions SET generation=lower(hex(randomblob(16))),next_sync=0",
            [],
        )
        .map_err(sql)?;
        tx.commit().map_err(sql)?;
        Ok(path.to_string_lossy().into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::tests::{sample, workspace};
    #[test]
    fn restore_is_atomic_closes_sessions_and_keeps_a_recoverable_copy() {
        let directory = std::env::temp_dir().join(format!("league-replay-backup-{}", id()));
        let mut original = Store::new(Connection::open_in_memory().unwrap()).unwrap();
        original.observe(Some(&sample("a", 1)), 1000).unwrap();
        let mut target = Store::new(Connection::open_in_memory().unwrap()).unwrap();
        target.observe(Some(&sample("b", 2)), 2000).unwrap();
        let before = target.backup_fingerprint().unwrap();
        assert_eq!(
            target
                .play_page(&PlayQuery::default())
                .unwrap()
                .summary
                .account_ids,
            vec!["HN1:b"]
        );
        let cursor = target
            .sync_workspace(ClientStatus::unavailable("offline", "test", 0), None)
            .unwrap()
            .cursor;
        let preview = target.backup_preview(&original, now()).unwrap();
        assert_eq!(preview.counts["participations"], 1);
        let copy = target
            .restore_snapshot(&original, &preview.fingerprint, &directory)
            .unwrap();
        let restored = workspace(&target);
        let changed = target
            .sync_workspace(
                ClientStatus::unavailable("offline", "test", 0),
                Some(cursor),
            )
            .unwrap();
        assert!(changed.cursor > cursor);
        assert!(changed.workspace.is_some());
        assert_eq!(changed.removed.len(), 1);
        assert_eq!(restored.matches[0].account_id, "HN1:a");
        assert_eq!(
            target
                .play_page(&PlayQuery::default())
                .unwrap()
                .summary
                .account_ids,
            vec!["HN1:a"]
        );
        assert_eq!(restored.matches[0].state, "pending");
        assert!(restored.sessions[0].ended_at.is_some());
        assert_eq!(restored.ui_revision, 1);
        assert!(target.save_ui(&LiveUiState::default(), 0).is_err());
        let (mut check, _) = Store::read_backup(std::fs::File::open(&copy).unwrap()).unwrap();
        assert_eq!(check.backup_fingerprint().unwrap(), before);
        check.observe(Some(&sample("a", 3)), 3000).unwrap();
        assert!(check
            .restore_snapshot(&original, &before, &directory)
            .is_err());
        std::fs::remove_file(copy).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
    #[test]
    fn cannot_replace_when_safety_copy_cannot_be_written() {
        let file = std::env::temp_dir().join(format!("league-replay-backup-{}", id()));
        std::fs::write(&file, "blocked directory").unwrap();
        let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
        let source = store.backup_snapshot().unwrap();
        let before = store.backup_fingerprint().unwrap();
        assert_eq!(
            store
                .restore_snapshot(&source, &before, &file)
                .err()
                .unwrap(),
            "backup.writeFailed"
        );
        assert_eq!(store.backup_fingerprint().unwrap(), before);
        std::fs::remove_file(file).unwrap();
    }

    #[test]
    fn failed_copy_rolls_back_the_entire_replacement() {
        let directory = std::env::temp_dir().join(format!("league-replay-rollback-{}", id()));
        let mut target = Store::new(Connection::open_in_memory().unwrap()).unwrap();
        target.observe(Some(&sample("a", 1)), 1000).unwrap();
        let mut invalid_source = Store::new(Connection::open_in_memory().unwrap()).unwrap();
        invalid_source.observe(Some(&sample("b", 2)), 2000).unwrap();
        invalid_source
            .0
            .pragma_update(None, "foreign_keys", false)
            .unwrap();
        invalid_source
            .0
            .execute("DELETE FROM accounts", [])
            .unwrap();
        let before = target.backup_fingerprint().unwrap();
        assert!(target
            .restore_snapshot(&invalid_source, &before, &directory)
            .is_err());
        assert_eq!(target.backup_fingerprint().unwrap(), before);
        let files: Vec<_> = std::fs::read_dir(&directory)
            .unwrap()
            .collect::<std::io::Result<_>>()
            .unwrap();
        assert_eq!(files.len(), 1);
        let (recovery, _) =
            Store::read_backup(std::fs::File::open(files[0].path()).unwrap()).unwrap();
        assert_eq!(recovery.backup_fingerprint().unwrap(), before);
        std::fs::remove_file(files[0].path()).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
