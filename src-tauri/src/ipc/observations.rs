use crate::ipc::CommandError;
use crate::{
    archive::SharedArchive,
    client::{client_error, lcu},
    domain::*,
    observer::SharedObserver,
};
use tauri::State;
#[tauri::command]
pub fn move_live_matches(
    observation_ids: Vec<String>,
    destination_id: Option<String>,
    title: String,
    archive: State<'_, SharedArchive>,
) -> Result<String, CommandError> {
    archive
        .lock()
        .map_err(|_| "library.storageError")?
        .move_matches(&observation_ids, destination_id.as_deref(), &title)
        .map_err(CommandError::from)
}
#[tauri::command]
pub fn sync_workspace(
    cursor: Option<f64>,
    observer: State<'_, SharedObserver>,
    archive: State<'_, SharedArchive>,
) -> Result<WorkspaceUpdate, CommandError> {
    let status = observer
        .status
        .lock()
        .map_err(|_| "client.Unavailable")?
        .clone();
    archive
        .lock()
        .map_err(|_| "library.storageError")?
        .sync_workspace(status, cursor)
        .map_err(CommandError::from)
}
#[tauri::command]
pub fn live_workspace(
    observer: State<'_, SharedObserver>,
    archive: State<'_, SharedArchive>,
) -> Result<LiveWorkspace, CommandError> {
    let status = observer
        .status
        .lock()
        .map_err(|_| "client.Unavailable")?
        .clone();
    archive
        .lock()
        .map_err(|_| "library.storageError")?
        .workspace(status)
        .map_err(CommandError::from)
}

#[tauri::command]
pub fn save_live_ui(
    ui: LiveUiState,
    revision: u32,
    archive: State<'_, SharedArchive>,
) -> Result<u32, CommandError> {
    archive
        .lock()
        .map_err(|_| "library.storageError")?
        .save_ui(&ui, revision)
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn end_live_session(
    session_id: String,
    archive: State<'_, SharedArchive>,
) -> Result<(), CommandError> {
    let client = tokio::task::spawn_blocking(lcu::discover)
        .await
        .map_err(|_| "client.Unavailable")?;
    match client {
        Ok(client) => {
            let sample = client.sample().await.map_err(client_error)?;
            if sample.queue_active {
                return Err("session.clientActive".into());
            }
        }
        Err(lcu::LcuError::Offline) => (),
        Err(error) => return Err(client_error(error).into()),
    }
    archive
        .lock()
        .map_err(|_| "library.storageError")?
        .end_session(&session_id, now())
        .map_err(CommandError::from)
}

#[tauri::command]
pub fn retract_manual_game(
    observation_id: String,
    archive: State<'_, SharedArchive>,
) -> Result<(), CommandError> {
    archive
        .lock()
        .map_err(|_| "library.storageError")?
        .retract_manual(&observation_id)
        .map_err(CommandError::from)
}

#[tauri::command]
pub fn rename_live_session(
    session_id: String,
    title: String,
    archive: State<'_, SharedArchive>,
) -> Result<(), CommandError> {
    archive
        .lock()
        .map_err(|_| "library.storageError")?
        .rename_session(&session_id, &title)
        .map_err(CommandError::from)
}

#[tauri::command]
pub fn merge_live_sessions(
    source_id: String,
    destination_id: String,
    archive: State<'_, SharedArchive>,
) -> Result<(), CommandError> {
    archive
        .lock()
        .map_err(|_| "library.storageError")?
        .merge_sessions(&source_id, &destination_id)
        .map_err(CommandError::from)
}

#[tauri::command]
pub fn split_live_session(
    session_id: String,
    segment_id: String,
    title: String,
    archive: State<'_, SharedArchive>,
) -> Result<(), CommandError> {
    archive
        .lock()
        .map_err(|_| "library.storageError")?
        .split_session(&session_id, &segment_id, &title)
        .map_err(CommandError::from)
}

#[tauri::command]
pub fn play_page(
    query: PlayQuery,
    archive: State<'_, SharedArchive>,
) -> Result<PlayPage, CommandError> {
    archive
        .lock()
        .map_err(|_| "library.storageError")?
        .play_page(&query)
        .map_err(CommandError::from)
}
