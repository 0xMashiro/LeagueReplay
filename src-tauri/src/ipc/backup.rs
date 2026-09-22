use crate::ipc::CommandError;
use crate::{
    archive::{backup::BackupPreview, SharedArchive},
    following::SharedFollowing,
    observer::SharedObserver,
    replays::SharedReplays,
};
use tauri::{Manager, State};

#[tauri::command]
pub async fn export_backup(
    archive: State<'_, SharedArchive>,
) -> Result<Option<String>, CommandError> {
    let archive = archive.inner().clone();
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("LeagueReplay archive", &["jsonl"])
            .set_file_name("LeagueReplay-backup.jsonl")
            .save_file()
        else {
            return Ok(None);
        };
        let snapshot = archive
            .lock()
            .map_err(|_| "library.storageError")?
            .backup_snapshot()?;
        snapshot.save_backup(&path)?;
        Ok(Some(path.to_string_lossy().into()))
    })
    .await
    .map_err(|_| "backup.writeFailed")?
    .map_err(CommandError::from)
}

#[derive(serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, export_to = "../../src/generated/library/")]
pub struct RestoreBackupRequest {
    path: String,
    fingerprint: String,
    source_fingerprint: String,
    merge: bool,
}
#[tauri::command]
pub async fn restore_backup(
    request: RestoreBackupRequest,
    app: tauri::AppHandle,
    archive: State<'_, SharedArchive>,
    observer: State<'_, SharedObserver>,
    following: State<'_, SharedFollowing>,
    replays: State<'_, SharedReplays>,
) -> Result<String, CommandError> {
    let (archive, observer, following, replays) = (
        archive.inner().clone(),
        observer.inner().clone(),
        following.inner().clone(),
        replays.inner().clone(),
    );
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let path = std::path::Path::new(&request.path);
        let (source, _) = crate::replays::package::read_backup(path)?;
        if source.backup_fingerprint()? != request.source_fingerprint {
            return Err("backup.changed".into());
        }
        let _observer = observer.gate.try_write().map_err(|_| "backup.busy")?;
        let _following = following.gate.try_lock().map_err(|_| "backup.busy")?;
        let status = observer.status.lock().map_err(|_| "backup.busy")?;
        if status.state != "offline"
            || crate::domain::now() as f64 - status.last_checked_at > 15_000.0
        {
            return Err("backup.offlineOnly".into());
        }
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|_| "backup.writeFailed")?
            .join("backups");
        replays.with_idle(|| {
            let mut store = archive.lock().map_err(|_| "library.storageError")?;
            if store.backup_fingerprint()? != request.fingerprint {
                return Err("backup.changed".into());
            }
            let merged = if request.merge {
                Some(store.merge_snapshot(&source)?)
            } else {
                None
            };
            if path
                .extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("zip"))
            {
                replays.restore_package_files(path, &source)?;
            }
            store.restore_snapshot(
                merged.as_ref().unwrap_or(&source),
                &request.fingerprint,
                &dir,
            )
        })
    })
    .await
    .map_err(|_| "backup.restoreFailed")?
    .map_err(CommandError::from)
}

#[derive(serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/generated/library/")]
pub struct BackupSelection {
    path: String,
    source_fingerprint: String,
    preview: BackupPreview,
}
#[tauri::command]
pub async fn choose_backup(
    archive: State<'_, SharedArchive>,
) -> Result<Option<BackupSelection>, CommandError> {
    let archive = archive.inner().clone();
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("LeagueReplay backup", &["jsonl", "zip"])
            .pick_file()
        else {
            return Ok(None);
        };
        let (source, date) = crate::replays::package::read_backup(&path)?;
        Ok(Some(BackupSelection {
            path: path.to_string_lossy().into(),
            source_fingerprint: source.backup_fingerprint()?,
            preview: archive
                .lock()
                .map_err(|_| "library.storageError")?
                .backup_preview(&source, date)?,
        }))
    })
    .await
    .map_err(|_| "backup.invalid")?
    .map_err(CommandError::from)
}
#[tauri::command]
pub async fn export_backup_package(
    archive: State<'_, SharedArchive>,
    replays: State<'_, SharedReplays>,
) -> Result<Option<String>, CommandError> {
    let (archive, replays) = (archive.inner().clone(), replays.inner().clone());
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("ZIP", &["zip"])
            .set_file_name("LeagueReplay-backup.zip")
            .save_file()
        else {
            return Ok(None);
        };
        replays.export_package(&archive, &path, None)?;
        Ok(Some(path.to_string_lossy().to_string()))
    })
    .await
    .map_err(|_| "backup.writeFailed")?
    .map_err(CommandError::from)
}
#[tauri::command]
pub async fn share_replay(
    id: String,
    archive: State<'_, SharedArchive>,
    replays: State<'_, SharedReplays>,
) -> Result<Option<String>, CommandError> {
    let (archive, replays) = (archive.inner().clone(), replays.inner().clone());
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("ZIP", &["zip"])
            .set_file_name(format!("{id}-review.zip"))
            .save_file()
        else {
            return Ok(None);
        };
        replays.export_package(&archive, &path, Some(&id))?;
        Ok(Some(path.to_string_lossy().to_string()))
    })
    .await
    .map_err(|_| "backup.writeFailed")?
    .map_err(CommandError::from)
}
