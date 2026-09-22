use crate::ipc::CommandError;
use crate::{
    archive::SharedArchive,
    library,
    replays::{ReplayJob, SharedReplays},
};
use tauri::State;
#[tauri::command]
pub fn replay_jobs(
    replays: State<'_, SharedReplays>,
    archive: State<'_, SharedArchive>,
) -> Result<Vec<ReplayJob>, CommandError> {
    replays.list(&archive).map_err(CommandError::from)
}
#[tauri::command]
pub fn download_replay(
    id: String,
    replays: State<'_, SharedReplays>,
    archive: State<'_, SharedArchive>,
) -> Result<(), CommandError> {
    let game = library::saved_review_game(id, None, &archive)?;
    replays
        .start(game, archive.inner().clone())
        .map_err(CommandError::from)
}
#[tauri::command]
pub async fn open_replay(
    id: String,
    replays: State<'_, SharedReplays>,
    archive: State<'_, SharedArchive>,
) -> Result<(), CommandError> {
    let game = library::saved_review_game(id, None, &archive)?;
    replays.open(&game).await.map_err(CommandError::from)
}
#[tauri::command]
pub fn reveal_replay(id: String, replays: State<'_, SharedReplays>) -> Result<(), CommandError> {
    replays.reveal(&id).map_err(CommandError::from)
}

#[tauri::command]
pub fn cancel_replay(id: String, replays: State<'_, SharedReplays>) -> Result<(), CommandError> {
    replays.cancel(&id).map_err(CommandError::from)
}

#[tauri::command]
pub fn remove_replay(
    id: String,
    replays: State<'_, SharedReplays>,
    archive: State<'_, SharedArchive>,
) -> Result<(), CommandError> {
    replays.remove(&id, &archive).map_err(CommandError::from)
}

#[tauri::command]
pub fn replay_directory(replays: State<'_, SharedReplays>) -> Result<String, CommandError> {
    Ok(replays.root()?.to_string_lossy().into())
}
#[tauri::command]
pub async fn choose_replay_directory(
    replays: State<'_, SharedReplays>,
) -> Result<Option<String>, CommandError> {
    let replays = replays.inner().clone();
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        rfd::FileDialog::new()
            .set_directory(replays.root()?)
            .pick_folder()
            .map(|path| replays.change_root(&path))
            .transpose()
    })
    .await
    .map_err(|_| "replay.storageError")?
    .map_err(CommandError::from)
}
#[tauri::command]
pub async fn import_replay(
    id: String,
    replays: State<'_, SharedReplays>,
    archive: State<'_, SharedArchive>,
) -> Result<bool, CommandError> {
    let game = library::saved_review_game(id, None, &archive)?;
    let (replays, archive) = (replays.inner().clone(), archive.inner().clone());
    tokio::task::spawn_blocking(move || -> Result<_, String> {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("ROFL", &["rofl"])
            .pick_file()
        else {
            return Ok(false);
        };
        replays.import(&path, &game, &archive)?;
        Ok(true)
    })
    .await
    .map_err(|_| "replay.storageError")?
    .map_err(CommandError::from)
}

#[tauri::command]
pub async fn seek_replay(
    id: String,
    at: f64,
    replays: State<'_, SharedReplays>,
    archive: State<'_, SharedArchive>,
) -> Result<(), CommandError> {
    let game = library::saved_review_game(id, None, &archive)?;
    replays.seek(&game, at).await.map_err(CommandError::from)
}
