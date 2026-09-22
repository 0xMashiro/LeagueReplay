use crate::ipc::CommandError;
use crate::{
    archive::SharedArchive,
    domain::{
        following::{FeedEntry, FollowingState},
        now,
    },
    following::SharedFollowing,
    library,
};
use tauri::State;

#[tauri::command]
pub fn following_state(
    service: State<'_, SharedFollowing>,
) -> Result<FollowingState, CommandError> {
    service.state().map_err(CommandError::from)
}

#[tauri::command]
pub async fn follow_player(
    server: String,
    puuid: String,
    archive: State<'_, SharedArchive>,
) -> Result<String, CommandError> {
    let page = library::player_history_page(server, puuid, 0).await?;
    archive
        .lock()
        .map_err(|_| "library.storageError")?
        .subscribe(&page, now())
        .map_err(CommandError::from)
}

#[tauri::command]
pub fn edit_following(
    id: String,
    label: String,
    paused: bool,
    archive: State<'_, SharedArchive>,
) -> Result<(), CommandError> {
    archive
        .lock()
        .map_err(|_| "library.storageError")?
        .edit_subscription(&id, &label, paused)
        .map_err(CommandError::from)
}

#[tauri::command]
pub fn unfollow_player(id: String, archive: State<'_, SharedArchive>) -> Result<(), CommandError> {
    archive
        .lock()
        .map_err(|_| "library.storageError")?
        .unfollow(&id)
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn sync_following(
    id: Option<String>,
    service: State<'_, SharedFollowing>,
) -> Result<(), CommandError> {
    service
        .sync(id.as_deref(), true)
        .await
        .map_err(CommandError::from)
}

#[tauri::command]
pub fn following_feed(
    id: Option<String>,
    unread: bool,
    start: u32,
    archive: State<'_, SharedArchive>,
) -> Result<Vec<FeedEntry>, CommandError> {
    archive
        .lock()
        .map_err(|_| "library.storageError")?
        .follow_feed(id.as_deref(), unread, start)
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn fill_following_history(
    id: String,
    following: State<'_, SharedFollowing>,
) -> Result<u32, CommandError> {
    following
        .fill_history(&id)
        .await
        .map_err(CommandError::from)
}
