use crate::ipc::CommandError;
use crate::{
    archive::SharedArchive,
    client::regions::Region,
    domain::review::{LibraryEntry, ReviewGame, SearchResult},
    library,
};
use tauri::State;
#[tauri::command]
pub async fn backfill_games(
    server: String,
    game_ids: Vec<String>,
    puuid: String,
    session_id: Option<String>,
    title: String,
    archive: State<'_, SharedArchive>,
) -> Result<(), CommandError> {
    if game_ids.is_empty()
        || game_ids.len() > 50
        || game_ids
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != game_ids.len()
    {
        return Err("game.invalidId".into());
    }
    let mut games = Vec::new();
    for game_id in game_ids {
        games.push(
            library::open_review_game(server.clone(), game_id, puuid.clone(), false, &archive)
                .await?,
        );
    }
    archive
        .lock()
        .map_err(|_| "library.storageError")?
        .import_many(&games, session_id.as_deref(), &title)
        .map_err(CommandError::from)
}
#[tauri::command]
pub async fn query_regions() -> Vec<Region> {
    library::query_regions().await
}
#[tauri::command]
pub async fn search_player(server: String, riot_id: String) -> Result<SearchResult, CommandError> {
    library::search_player(server, riot_id)
        .await
        .and_then(crate::matches::search_result)
        .map_err(CommandError::from)
}
#[tauri::command]
pub async fn search_current_player() -> Result<SearchResult, CommandError> {
    library::search_current_player()
        .await
        .and_then(crate::matches::search_result)
        .map_err(CommandError::from)
}
#[tauri::command]
pub async fn player_history_page(
    server: String,
    puuid: String,
    start: u32,
) -> Result<SearchResult, CommandError> {
    library::player_history_page(server, puuid, start)
        .await
        .and_then(crate::matches::search_result)
        .map_err(CommandError::from)
}
#[tauri::command]
pub async fn open_review_game(
    server: String,
    game_id: String,
    puuid: String,
    refresh: bool,
    archive: State<'_, SharedArchive>,
) -> Result<ReviewGame, CommandError> {
    library::open_review_game(server, game_id, puuid, refresh, &archive)
        .await
        .and_then(crate::matches::review_game)
        .map_err(CommandError::from)
}
#[tauri::command]
pub fn saved_review_game(
    id: String,
    puuid: Option<String>,
    archive: State<'_, SharedArchive>,
) -> Result<ReviewGame, CommandError> {
    library::saved_review_game(id, puuid, &archive)
        .and_then(crate::matches::review_game)
        .map_err(CommandError::from)
}
#[tauri::command]
pub fn list_library(
    start: u32,
    filter: String,
    query: Option<String>,
    archive: State<'_, SharedArchive>,
) -> Result<Vec<LibraryEntry>, CommandError> {
    library::list_library(start, filter, query.unwrap_or_default(), &archive)
        .map_err(CommandError::from)
}
#[tauri::command]
pub fn bookmark_review_game(
    id: String,
    bookmarked: bool,
    archive: State<'_, SharedArchive>,
) -> Result<(), CommandError> {
    library::bookmark_review_game(id, bookmarked, &archive).map_err(CommandError::from)
}
#[tauri::command]
pub fn backfill_review_game(
    id: String,
    puuid: String,
    session_id: Option<String>,
    archive: State<'_, SharedArchive>,
) -> Result<(), CommandError> {
    library::backfill_review_game(id, puuid, session_id, &archive).map_err(CommandError::from)
}

#[tauri::command]
pub async fn ensure_review_timeline(
    id: String,
    puuid: String,
    archive: State<'_, SharedArchive>,
) -> Result<crate::domain::review::ReviewGame, CommandError> {
    let mut game = library::saved_review_game(id, Some(puuid), &archive)?;
    if game.timeline.is_some() {
        return crate::matches::review_game(game).map_err(CommandError::from);
    }
    let fresh = library::fetch_game(
        &game.server,
        &game.detail["gameId"]
            .as_u64()
            .ok_or("game.invalidId")?
            .to_string(),
        &game.account.puuid,
    )
    .await?;
    game.timeline = fresh.timeline;
    game.timeline_error = fresh.timeline_error;
    let mut store = archive.lock().map_err(|_| "library.storageError")?;
    store.save_review(&game, crate::domain::now())?;
    // Return the merged record: a concurrent background fetch may have succeeded.
    let saved = store
        .review(&game.id, Some(&game.account.puuid))?
        .ok_or("library.invalidData")?;
    crate::matches::review_game(saved).map_err(CommandError::from)
}
