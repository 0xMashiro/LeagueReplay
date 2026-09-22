use crate::domain::now;
use crate::{
    archive::SharedArchive,
    client::{
        client_error, connect as client,
        lcu::{self, LocalClient},
        regions, sgp as gateway,
    },
    domain::Account,
};
use crate::{
    domain::{review as model, validate_game_id},
    matches as data,
};
use model::{HistoryPage, StoredReview};
use serde_json::json;

fn validate_puuid(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
        || value == "00000000-0000-0000-0000-000000000000"
    {
        Err("player.invalidId".into())
    } else {
        Ok(())
    }
}

pub async fn query_regions() -> Vec<regions::Region> {
    regions::list(client().await.ok().as_ref())
}

// This identity does not assert membership in a region. Only returned match rows do.
fn query_account(server: &str, puuid: &str) -> Result<Account, String> {
    validate_puuid(puuid)?;
    let platform = regions::platform(server)?;
    Ok(Account {
        id: format!("{platform}:{puuid}"),
        platform,
        puuid: puuid.into(),
        summoner_id: String::new(),
        riot_id: puuid.into(),
    })
}

async fn resolve(client: &LocalClient, server: &str, riot_id: &str) -> Result<Account, String> {
    let (name, tag) = riot_id
        .trim()
        .rsplit_once('#')
        .filter(|(name, tag)| {
            !name.trim().is_empty()
                && !tag.trim().is_empty()
                && name.len() <= 128
                && tag.len() <= 32
        })
        .ok_or("player.enterRiotId")?;
    let platform = regions::platform(server)?;
    if regions::current(client) != server {
        regions::check_history_access(&regions::current(client), server)?;
    }
    // Resolve identity independently of the target region's summoner/profile endpoint.
    let aliases = match client.aliases(name, tag).await {
        Err(error) if regions::current(client) != server => {
            return Err(if error == lcu::LcuError::NotFound {
                "player.riotClientUnavailable".into()
            } else {
                client_error(error)
            });
        }
        result => result,
    };
    if let Ok(aliases) = aliases {
        let candidates: Vec<_> = aliases
            .as_array()
            .ok_or("game.invalidData")?
            .iter()
            .filter(|v| {
                v["alias"]["game_name"]
                    .as_str()
                    .is_some_and(|v| v.eq_ignore_ascii_case(name))
                    && v["alias"]["tag_line"]
                        .as_str()
                        .is_some_and(|v| v.eq_ignore_ascii_case(tag))
            })
            .collect();
        if candidates.len() > 1 {
            return Err("player.ambiguous".into());
        }
        if let Some(alias) = candidates.first() {
            let puuid = alias["puuid"].as_str().ok_or("player.invalidId")?;
            validate_puuid(puuid)?;
            if regions::current(client) == server {
                let raw = client
                    .get(&format!(
                        "/lol-summoner/v2/summoners/puuid/{}",
                        gateway::url_segment(puuid)
                    ))
                    .await
                    .map_err(client_error)?;
                return lcu::parse_account(&platform, &raw).map_err(client_error);
            }
            let mut account = query_account(server, puuid)?;
            account.riot_id = format!("{name}#{tag}");
            return Ok(account);
        }
        return Err("player.notFound".into());
    }
    // Some installations do not expose the Riot Client port. LCU aliases remain local-region only.
    let raw = client
        .post(
            "/lol-summoner/v1/summoners/aliases",
            &json!([{"gameName":name,"tagLine":tag}]),
        )
        .await
        .map_err(client_error)?;
    let players = raw.as_array().ok_or("player.notFound")?;
    if players.len() != 1 {
        return Err("player.notFound".into());
    }
    lcu::parse_account(&platform, &players[0]).map_err(client_error)
}

async fn history(
    client: &LocalClient,
    server: &str,
    account: Account,
    start: u32,
) -> Result<HistoryPage, String> {
    if start > 10000 || !start.is_multiple_of(20) {
        return Err("query.invalidPage".into());
    }
    validate_puuid(&account.puuid)?;
    let (games, source) = if regions::current(client) == server {
        let raw = client
            .get(&format!(
                "/lol-match-history/v1/products/lol/{}/matches?begIndex={start}&endIndex={}",
                gateway::url_segment(&account.puuid),
                start + 19
            ))
            .await
            .map_err(client_error)?;
        (
            raw["games"]["games"]
                .as_array()
                .ok_or("game.invalidData")?
                .clone(),
            "lcu",
        )
    } else {
        let gateway = gateway::Gateway::connect(client, server).await?;
        let raw = gateway.history(&account.puuid, start).await?;
        (
            raw["games"].as_array().ok_or("game.invalidData")?.clone(),
            "sgp",
        )
    };
    history_page(server, account, start, source, &games)
}

fn history_page(
    server: &str,
    mut account: Account,
    start: u32,
    source: &str,
    raw: &[serde_json::Value],
) -> Result<HistoryPage, String> {
    if regions::platform(server)? != account.platform {
        return Err("game.identityMismatch".into());
    }
    let mut games = Vec::with_capacity(raw.len());
    for value in raw {
        if let Ok(game) = data::summary(server, value) {
            let belongs = game["participantIdentities"]
                .as_array()
                .is_some_and(|identities| {
                    identities.iter().any(|identity| {
                        identity["player"]["puuid"].as_str() == Some(&account.puuid)
                    })
                });
            if belongs && data::project(&game).is_ok() {
                games.push(game);
            }
        }
    }
    let skipped_games = (raw.len() - games.len()) as u32;
    if !raw.is_empty() && games.is_empty() {
        return Err("game.historyUnreadable".into());
    }
    // Enrich a PUUID-only identity from validated matches; empty results invent no profile.
    if account.summoner_id.is_empty() {
        if let Some(found) = games
            .iter()
            .find_map(|game| data::account_in_game(&account.platform, &account.puuid, game).ok())
        {
            account.summoner_id = found.summoner_id;
            if account.riot_id == account.puuid {
                account.riot_id = found.riot_id;
            }
        }
    }
    Ok(HistoryPage {
        account,
        server: server.into(),
        // Server offsets count all returned rows, including the unreadable ones.
        has_more: raw.len() >= 20,
        skipped_games,
        games,
        start,
        source: source.into(),
    })
}

pub async fn search_player(server: String, riot_id: String) -> Result<HistoryPage, String> {
    let client = client().await?;
    let account = resolve(&client, &server, &riot_id).await?;
    history(&client, &server, account, 0).await
}

pub async fn search_current_player() -> Result<HistoryPage, String> {
    let client = client().await?;
    let before = client.sample().await.map_err(client_error)?;
    let result = history(
        &client,
        &regions::current(&client),
        before.account.clone(),
        0,
    )
    .await?;
    if client.sample().await.map_err(client_error)?.account.id != before.account.id {
        return Err("client.IdentityChanged".into());
    }
    Ok(result)
}

pub async fn player_history_page(
    server: String,
    puuid: String,
    start: u32,
) -> Result<HistoryPage, String> {
    validate_puuid(&puuid)?;
    let client = client().await?;
    let platform = regions::platform(&server)?;
    let account = if regions::current(&client) == server {
        lcu::parse_account(
            &platform,
            &client
                .get(&format!(
                    "/lol-summoner/v2/summoners/puuid/{}",
                    gateway::url_segment(&puuid)
                ))
                .await
                .map_err(client_error)?,
        )
        .map_err(client_error)?
    } else {
        regions::check_history_access(&regions::current(&client), &server)?;
        query_account(&server, &puuid)?
    };
    history(&client, &server, account, start).await
}

pub async fn fetch_game(server: &str, game_id: &str, puuid: &str) -> Result<StoredReview, String> {
    validate_game_id(game_id)?;
    validate_puuid(puuid)?;
    let client = client().await?;
    let platform = regions::platform(server)?;
    let (detail, timeline, source) = if regions::current(&client) == server {
        let detail = data::summary(server, &client.detail(game_id).await.map_err(client_error)?)?;
        let timeline = client
            .timeline(game_id)
            .await
            .map_err(client_error)
            .and_then(|v| data::lcu_timeline(server, game_id, v));
        (detail, timeline, "lcu")
    } else {
        let gateway = gateway::Gateway::connect(&client, server).await?;
        let detail = data::summary(server, &gateway.game(game_id, false).await?)?;
        let timeline = gateway
            .game(game_id, true)
            .await
            .and_then(|v| data::sgp_timeline(server, game_id, v));
        (detail, timeline, "sgp")
    };
    if detail["gameId"].as_u64().map(|v| v.to_string()).as_deref() != Some(game_id) {
        return Err("game.identityMismatch".into());
    }
    let account = data::account_in_game(&platform, puuid, &detail)?;
    let id = format!("{platform}_{game_id}");
    crate::archive::validate_detail(&id, &account, &detail).map_err(|_| "game.invalidData")?;
    let (timeline, timeline_error) = match timeline {
        Ok(v) => (Some(v), None),
        Err(e) => (None, Some(e)),
    };
    Ok(StoredReview {
        id,
        server: server.into(),
        account,
        detail,
        timeline,
        source: source.into(),
        timeline_error,
        bookmarked: false,
    })
}

pub async fn open_review_game(
    server: String,
    game_id: String,
    puuid: String,
    refresh: bool,
    archive: &SharedArchive,
) -> Result<StoredReview, String> {
    validate_game_id(&game_id)?;
    validate_puuid(&puuid)?;
    let id = format!("{}_{game_id}", regions::platform(&server)?);
    if !refresh {
        let mut store = archive.lock().map_err(|_| "library.storageError")?;
        if let Some(game) = store.visit_review(&id, Some(&puuid), now())? {
            return Ok(game);
        }
    }
    let game = fetch_game(&server, &game_id, &puuid).await?;
    let mut store = archive.lock().map_err(|_| "library.storageError")?;
    store.save_review(&game, now())?;
    store
        .review(&id, Some(&puuid))?
        .ok_or("library.notFound".into())
}

pub fn saved_review_game(
    id: String,
    puuid: Option<String>,
    archive: &SharedArchive,
) -> Result<StoredReview, String> {
    let mut store = archive.lock().map_err(|_| "library.storageError")?;
    store
        .visit_review(&id, puuid.as_deref(), now())?
        .ok_or("library.notFound".into())
}

pub fn list_library(
    start: u32,
    filter: String,
    query: String,
    archive: &SharedArchive,
) -> Result<Vec<model::LibraryEntry>, String> {
    archive
        .lock()
        .map_err(|_| "library.storageError")?
        .library(start, &filter, &query)
}

pub fn bookmark_review_game(
    id: String,
    bookmarked: bool,
    archive: &SharedArchive,
) -> Result<(), String> {
    archive
        .lock()
        .map_err(|_| "library.storageError")?
        .bookmark(&id, bookmarked)
}

pub fn backfill_review_game(
    id: String,
    puuid: String,
    session_id: Option<String>,
    archive: &SharedArchive,
) -> Result<(), String> {
    let mut store = archive.lock().map_err(|_| "library.storageError")?;
    let game = store.review(&id, Some(&puuid))?.ok_or("library.notFound")?;
    store.import(
        &game.account,
        &game.detail,
        game.timeline.as_ref(),
        session_id.as_deref(),
        now(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cross_region_history_needs_no_profile_and_preserves_empty_results() {
        let mut game = row(1);
        game["platformId"] = json!("KR");
        let account = query_account("KR", "player").unwrap();
        let empty = history_page("KR", account.clone(), 20, "sgp", &[]).unwrap();
        assert!(empty.games.is_empty() && !empty.has_more);
        assert_eq!(empty.account.id, "KR:player");
        assert!(empty.account.summoner_id.is_empty());
        let found = history_page("KR", account.clone(), 0, "sgp", &[game.clone()]).unwrap();
        assert_eq!(found.account.riot_id, "Player#TEST");
        assert_eq!(found.account.summoner_id, "1");
        let mut named = account;
        named.riot_id = "CurrentName#TEST".into();
        let found = history_page("KR", named, 0, "sgp", &[game]).unwrap();
        assert_eq!(found.account.riot_id, "CurrentName#TEST");
        // A shared host returning another region's match must not relabel it as Korea.
        assert!(history_page(
            "KR",
            query_account("KR", "player").unwrap(),
            0,
            "sgp",
            &[row(1)]
        )
        .is_err());
    }

    #[tokio::test]
    #[ignore = "requires an explicitly selected target region and Riot ID; read-only, no downloads"]
    async fn live_cross_region_history() {
        let server = std::env::var("LEAGUEREPLAY_TEST_SERVER").expect("set target server ID");
        let riot_id = std::env::var("LEAGUEREPLAY_TEST_RIOT_ID").expect("set target Riot ID");
        let client = client().await.expect("signed-in client");
        let source = regions::current(&client);
        assert_ne!(
            source, server,
            "choose a different server for cross-region validation"
        );
        regions::check_history_access(&source, &server)
            .expect("same operator and configured routes");
        let account = resolve(&client, &server, &riot_id)
            .await
            .expect("resolve identity");
        let page = history(&client, &server, account, 0)
            .await
            .expect("history response");
        assert_eq!(page.skipped_games, 0);
        assert!(
            !page.games.is_empty(),
            "empty response does not verify cross-region match access"
        );
        let first = &page.games[0];
        let game_id = first["gameId"].as_u64().unwrap().to_string();
        let gateway = gateway::Gateway::connect(&client, &server).await.unwrap();
        let detail = data::summary(&server, &gateway.game(&game_id, false).await.unwrap()).unwrap();
        assert_eq!(detail["gameId"], first["gameId"]);
        data::account_in_game(&page.account.platform, &page.account.puuid, &detail).unwrap();
        data::sgp_timeline(
            &server,
            &game_id,
            gateway.game(&game_id, true).await.unwrap(),
        )
        .unwrap();
        println!("cross-region: source={source}, target={server}, games={}, detail_and_timeline_verified=true", page.games.len());
    }

    fn account() -> Account {
        Account {
            id: "EUN1:player".into(),
            platform: "EUN1".into(),
            puuid: "player".into(),
            summoner_id: "1".into(),
            riot_id: "Player#TEST".into(),
        }
    }
    fn row(id: u32) -> serde_json::Value {
        json!({"platformId":"EUN1", "gameId":id, "gameDuration":1200, "gameCreation":1000,
            "participantIdentities":[{"participantId":1,"player":{"puuid":"player","summonerId":1,"gameName":"Player","tagLine":"TEST"}}],
            "participants":[{"participantId":1,"championId":103,"stats":{"win":true}}]})
    }
    #[test]
    fn partial_history_keeps_server_pagination_and_valid_local_games() {
        let mut rows: Vec<_> = (1..=20).map(row).collect();
        rows[3] = serde_json::Value::Null;
        let page = history_page("EUN1", account(), 20, "lcu", &rows).unwrap();
        assert_eq!(
            (page.games.len(), page.skipped_games, page.start),
            (19, 1, 20)
        );
        assert!(page.has_more);
        assert!(regions::endpoint("EUN1", true).is_err());
        let result = data::search_result(page).unwrap();
        assert_eq!((result.start, result.skipped_games), (20, 1));
        assert!(result.has_more);
        assert_eq!(result.source, "lcu");
        assert_eq!(result.account.puuid, "player");
        assert_eq!(result.games[0].game_id, "1");
        let wire = serde_json::to_value(result).unwrap();
        let first = &wire["games"][0];
        assert_eq!(first["game"]["participants"][0]["puuid"], "player");
        assert_eq!(first["game"]["participants"][0]["championId"], 103);
        assert!(first.get("participantIdentities").is_none());
        assert!(first.get("detail").is_none());
    }

    #[test]
    fn history_skips_unprojectable_rosters_without_shifting_server_offsets() {
        let mut invalid = row(2);
        invalid["participants"][0]["participantId"] = json!(0);
        let page = history_page("EUN1", account(), 40, "lcu", &[row(1), invalid.clone()]).unwrap();
        let result = data::search_result(page).unwrap();
        assert_eq!(
            (result.start, result.skipped_games, result.games.len()),
            (40, 1, 1)
        );
        assert_eq!(result.games[0].game_id, "1");
        assert!(
            matches!(history_page("EUN1", account(), 40, "lcu", &[invalid]),
            Err(error) if error == "game.historyUnreadable")
        );
    }
    #[test]
    fn history_rejects_wrong_identities_and_distinguishes_empty_from_unreadable() {
        let mut wrong_region = row(2);
        wrong_region["platformId"] = json!("NA1");
        let mut wrong_player = row(3);
        wrong_player["participantIdentities"][0]["player"]["puuid"] = json!("someone-else");
        let page = history_page(
            "EUN1",
            account(),
            0,
            "sgp",
            &[
                json!({"metadata":{"match_id":"EUN1_1"},"json":row(1)}),
                wrong_region,
                wrong_player,
                json!({"metadata":{"match_id":"EUN1_4"}}),
            ],
        )
        .unwrap();
        assert_eq!((page.games.len(), page.skipped_games), (1, 3));
        assert!(!page.has_more);
        assert!(
            matches!(history_page("EUN1", account(), 0, "lcu", &[serde_json::Value::Null]),
            Err(e) if e == "game.historyUnreadable")
        );
        let empty = history_page("EUN1", account(), 0, "lcu", &[]).unwrap();
        assert!(empty.games.is_empty() && !empty.has_more && empty.skipped_games == 0);
        assert!(history_page("NA1", account(), 0, "lcu", &[row(1)]).is_err());
    }
    #[test]
    fn unconfigured_local_region_survives_archive_and_account_shortcuts() {
        let mut store =
            crate::archive::Store::new(rusqlite::Connection::open_in_memory().unwrap()).unwrap();
        let game = StoredReview {
            id: "EUN1_1".into(),
            server: "EUN1".into(),
            account: account(),
            detail: row(1),
            timeline: Some(data::lcu_timeline("EUN1", "1", json!({"frames":[]})).unwrap()),
            source: "lcu".into(),
            timeline_error: None,
            bookmarked: false,
        };
        store.save_review(&game, 1000).unwrap();
        let cached = store
            .cached_review("EUN1_1", Some("player"))
            .unwrap()
            .unwrap();
        assert_eq!(cached.server, "EUN1");
        let mut ui = crate::domain::LiveUiState::default();
        ui.my_accounts.push(crate::domain::SavedAccount {
            server: "EUN1".into(),
            account: account(),
        });
        store.save_ui(&ui, 0).unwrap();
        let mut backup = Vec::new();
        store.write_backup(&mut backup).unwrap();
        crate::archive::Store::read_backup(backup.as_slice()).unwrap();
    }
    #[tokio::test]
    #[ignore = "requires a logged-in local client; only read-only identity/history requests"]
    async fn live_query_smoke() {
        let client = client().await.unwrap();
        let current = client.sample().await.unwrap();
        let server = regions::current(&client);
        let found = resolve(&client, &server, &current.account.riot_id)
            .await
            .unwrap();
        assert!(found.puuid == current.account.puuid);
        let result = history(&client, &server, found, 0).await.unwrap();
        println!(
            "query: server={server}, source={}, games={}",
            result.source,
            result.games.len()
        );
        let gateway = gateway::Gateway::connect(&client, &server).await.unwrap();
        let raw = gateway.history(&current.account.puuid, 0).await.unwrap();
        let games = raw["games"].as_array().unwrap();
        for game in games {
            data::summary(&server, game).unwrap();
        }
        println!("gateway: games={}", games.len());
        if let Some(game) = games.first() {
            let id = game["json"]["gameId"].as_u64().unwrap().to_string();
            data::summary(&server, &gateway.game(&id, false).await.unwrap()).unwrap();
            let timeline =
                data::sgp_timeline(&server, &id, gateway.game(&id, true).await.unwrap()).unwrap();
            println!(
                "gateway timeline: frames={}",
                timeline["frames"].as_array().unwrap().len()
            );
        }
        let install = client
            .get("/lol-patch/v1/products/league_of_legends/install-location")
            .await
            .unwrap();
        println!(
            "install location: string={}, object_keys={:?}",
            install.is_string(),
            install.as_object().map(|v| v.keys().collect::<Vec<_>>())
        );
        let config = client.get("/lol-replays/v1/configuration").await.unwrap();
        let locale = client.get("/riotclient/region-locale").await.unwrap();
        println!("playback: version={}, logged_in={:?}, enabled={:?}, playing={:?}, replaying={:?}, patching={:?}, locale_present={}",config["gameVersion"].as_str().unwrap_or("unknown"),config["isLoggedIn"].as_bool(),config["isReplaysEnabled"].as_bool(),config["isPlayingGame"].as_bool(),config["isPlayingReplay"].as_bool(),config["isPatching"].as_bool(),locale["locale"].as_str().is_some());
        let root = std::path::Path::new(install["gameInstallRoot"].as_str().unwrap());
        let executable = std::path::Path::new(install["gameExecutablePath"].as_str().unwrap());
        let executable = if executable.is_absolute() {
            executable.to_path_buf()
        } else {
            root.join(executable)
        };
        println!(
            "playback install: root_exists={}, executable_exists={}",
            root.is_dir(),
            executable.is_file()
        );
    }
}
