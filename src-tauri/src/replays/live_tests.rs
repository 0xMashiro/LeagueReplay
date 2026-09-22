//! Opt-in acceptance. Only the explicitly gated install step writes to the user's archive.
use super::*;
use crate::{client::regions, library, matches};

#[test]
#[ignore = "reads installed games and LEAGUEREPLAY_TEST_OUTPUT; no LCU calls, login, network or process launch"]
fn live_offline_playback_preflight() {
    let output = PathBuf::from(std::env::var_os("LEAGUEREPLAY_TEST_OUTPUT").unwrap());
    let game: StoredReview =
        serde_json::from_slice(&std::fs::read(output.join("compatible-review.json")).unwrap())
            .unwrap();
    let replay = files::path(&output, &game.id).unwrap();
    let inspected = files::inspect(&replay, &game).unwrap();
    let command =
        playback::prepare_direct(&game.server, &replay, &inspected.game_version, None).unwrap();
    assert!(PathBuf::from(command.get_program()).is_file());
    assert_eq!(
        command.get_args().next(),
        Some(replay.canonicalize().unwrap().as_os_str())
    );
    assert!(command.get_current_dir().unwrap().is_dir());
    println!("offline playback preflight: installed_game=true, version_compatible=true, replay_argument_verified=true, lcu_calls=0, spawned=false");
}

#[test]
#[ignore = "copies the verified test replay into an explicitly selected existing archive; requires LEAGUEREPLAY_ACCEPTANCE_INSTALL=1"]
fn install_compatible_replay_for_manual_acceptance() {
    assert_eq!(
        std::env::var("LEAGUEREPLAY_ACCEPTANCE_INSTALL").as_deref(),
        Ok("1")
    );
    let output = PathBuf::from(std::env::var_os("LEAGUEREPLAY_TEST_OUTPUT").unwrap());
    let database = PathBuf::from(std::env::var_os("LEAGUEREPLAY_ACCEPTANCE_ARCHIVE").unwrap());
    assert!(
        database.is_absolute() && database.is_file(),
        "existing archive required"
    );
    let game: StoredReview =
        serde_json::from_slice(&std::fs::read(output.join("compatible-review.json")).unwrap())
            .unwrap();
    let source = files::path(&output, &game.id).unwrap();
    files::inspect(&source, &game).unwrap();
    let archive = Arc::new(Mutex::new(
        crate::archive::Store::new(
            rusqlite::Connection::open_with_flags(
                &database,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE,
            )
            .unwrap(),
        )
        .unwrap(),
    ));
    let snapshot = || {
        serde_json::to_value(
            archive
                .lock()
                .unwrap()
                .workspace(crate::domain::ClientStatus::unavailable(
                    "offline",
                    "acceptance",
                    0,
                ))
                .unwrap(),
        )
        .unwrap()
    };
    let before = snapshot();
    let service = Replays::new(database.parent().unwrap().join("replays")).unwrap();
    let destination = files::path(&service.root().unwrap(), &game.id).unwrap();
    if !destination.exists() {
        let temporary = files::PartialFile(
            service
                .root()
                .unwrap()
                .join(format!("{}.part", uuid::Uuid::new_v4())),
        );
        std::fs::copy(&source, &temporary.0).unwrap();
        files::inspect(&temporary.0, &game).unwrap();
        std::fs::hard_link(&temporary.0, &destination).unwrap();
    }
    files::inspect(&destination, &game).unwrap();
    archive
        .lock()
        .unwrap()
        .save_review(&game, crate::domain::now())
        .unwrap();
    service.finish(&game, &archive, &destination).unwrap();
    assert!(archive
        .lock()
        .unwrap()
        .library(0, "downloaded", "")
        .unwrap()
        .iter()
        .any(|row| row.id == game.id));
    assert!(service
        .list(&archive)
        .unwrap()
        .iter()
        .any(|job| job.id == game.id && job.state == "ready"));
    let after = snapshot();
    for field in ["matches", "sessions", "ui", "uiRevision"] {
        assert_eq!(
            before[field], after[field],
            "existing {field} must not change"
        );
    }
    println!("manual acceptance ready: library_entry=true, replay_validated=true, existing_play_and_notes_unchanged=true");
}

#[tokio::test]
#[ignore = "opens the compatible test replay in the game; requires LEAGUEREPLAY_TEST_OUTPUT and LEAGUEREPLAY_TEST_LAUNCH=1"]
async fn live_replay_playback_smoke() {
    assert_eq!(
        std::env::var("LEAGUEREPLAY_TEST_LAUNCH").as_deref(),
        Ok("1")
    );
    let output = PathBuf::from(
        std::env::var_os("LEAGUEREPLAY_TEST_OUTPUT").expect("explicit test output directory"),
    );
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output.join("compatible-review.json")).unwrap())
            .unwrap();
    let server = saved["server"].as_str().unwrap();
    assert!(["TENCENT_HN1", "TENCENT_HN10"].contains(&server));
    let game = library::fetch_game(
        server,
        &saved["detail"]["gameId"].as_u64().unwrap().to_string(),
        saved["account"]["puuid"].as_str().unwrap(),
    )
    .await
    .unwrap();
    let http = reqwest::Client::builder()
        .no_proxy()
        .danger_accept_invalid_certs(true) // Only the fixed local Replay API below.
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap();
    assert!(
        !http
            .get("https://127.0.0.1:2999/replay/game")
            .send()
            .await
            .is_ok_and(|r| r.status().is_success()),
        "another replay is already running"
    );
    let service = Replays::new(output).unwrap();
    service
        .open(&game)
        .await
        .expect("real application playback path");
    println!("playback launch accepted; waiting for a loaded replay with advancing time");
    let mut first_time = None;
    for _ in 0..30 {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let Ok(response) = http
            .get("https://127.0.0.1:2999/replay/playback")
            .send()
            .await
        else {
            continue;
        };
        if !response.status().is_success() {
            continue;
        }
        let state: serde_json::Value = response.json().await.unwrap();
        let Some(time) = state["time"].as_f64() else {
            continue;
        };
        let Some(length) = state["length"].as_f64().filter(|v| *v > 0.0) else {
            continue;
        };
        if first_time.is_some_and(|before| time > before + 0.2) {
            assert_eq!(service.open(&game).await, Err("replay.clientBusy".into()));
            println!("playback verified: timeline_advancing=true, duration_seconds={length:.1}");
            println!("repeat launch: rejected while the real replay process is running");
            return;
        }
        first_time.get_or_insert(time);
    }
    panic!("playback could not be verified: Replay API unavailable or timeline not advancing");
}

#[tokio::test]
#[ignore = "uses HN1/HN10 official services and writes LEAGUEREPLAY_TEST_OUTPUT; does not launch"]
async fn live_compatible_replay_download_smoke() {
    let output = PathBuf::from(
        std::env::var_os("LEAGUEREPLAY_TEST_OUTPUT").expect("explicit test output directory"),
    );
    let client = connect().await.unwrap();
    let server = regions::current(&client);
    assert!(["TENCENT_HN1", "TENCENT_HN10"].contains(&server.as_str()));
    let config = client.get("/lol-replays/v1/configuration").await.unwrap();
    let version = config["gameVersion"].as_str().unwrap();
    let patch = files::patch(version).unwrap();
    let current = library::search_current_player().await.unwrap();
    let mut candidate = current
        .games
        .iter()
        .find(|g| g["gameVersion"].as_str().and_then(files::patch) == Some(patch))
        .map(|g| {
            (
                g["gameId"].as_u64().unwrap().to_string(),
                current.account.puuid.clone(),
            )
        });
    let mut checked_players = 0;
    if candidate.is_none() {
        let id = current.games.first().expect("recent match")["gameId"]
            .as_u64()
            .unwrap()
            .to_string();
        let first = library::fetch_game(&server, &id, &current.account.puuid)
            .await
            .unwrap();
        let gateway = Gateway::connect(&client, &server).await.unwrap();
        let peers = first.detail["participantIdentities"].as_array().unwrap();
        for peer in peers
            .iter()
            .filter_map(|p| p["player"]["puuid"].as_str())
            .filter(|p| *p != current.account.puuid)
            .take(3)
        {
            checked_players += 1;
            let history = gateway.history(peer, 0).await.unwrap();
            let games = history["games"].as_array().unwrap();
            for raw in games {
                let detail = matches::summary(&server, raw).unwrap();
                if detail["gameVersion"].as_str().and_then(files::patch) == Some(patch) {
                    candidate = Some((
                        detail["gameId"].as_u64().unwrap().to_string(),
                        peer.to_owned(),
                    ));
                    break;
                }
            }
            if candidate.is_some() {
                break;
            }
        }
    }
    println!(
        "compatible lookup: client_version={version}, peer_histories={checked_players}, found={}",
        candidate.is_some()
    );
    let (id, puuid) = candidate.expect("no compatible match in the bounded recent histories");
    let game = library::fetch_game(&server, &id, &puuid).await.unwrap();
    // Exercise public player search and the next page using the identity returned in this match.
    let result = library::search_player(server.clone(), game.account.riot_id.clone())
        .await
        .unwrap();
    assert_eq!(result.account.puuid, puuid);
    let next = library::player_history_page(server, puuid, 20)
        .await
        .unwrap();
    println!(
        "other-player query: identity_verified=true, first_page={}, next_page={}",
        result.games.len(),
        next.games.len()
    );
    let archive = Arc::new(Mutex::new(
        crate::archive::Store::new(rusqlite::Connection::open_in_memory().unwrap()).unwrap(),
    ));
    archive
        .lock()
        .unwrap()
        .save_review(&game, crate::domain::now())
        .unwrap();
    let service = Replays::new(output.clone()).unwrap();
    service.start(game.clone(), archive.clone()).unwrap();
    for _ in 0..180 {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let job = service
            .list(&archive)
            .unwrap()
            .into_iter()
            .find(|j| j.id == game.id)
            .unwrap();
        if job.state == "ready" {
            assert_eq!(job.version.as_deref().and_then(files::patch), Some(patch));
            std::fs::write(
                output.join("compatible-review.json"),
                serde_json::to_vec(&game).unwrap(),
            )
            .unwrap();
            println!(
                "compatible replay: validated=true, bytes={}, version={}",
                job.received,
                job.version.unwrap()
            );
            return;
        }
        assert_ne!(job.state, "error", "download failed: {:?}", job.error);
    }
    panic!("compatible replay download timed out");
}
