use super::*;
use rusqlite::Connection;
use serde_json::{json, Value};
pub(super) fn account(name: &str) -> Account {
    Account {
        id: format!("HN1:{name}"),
        platform: "HN1".into(),
        puuid: name.into(),
        summoner_id: if name == "a" { "1" } else { "2" }.into(),
        riot_id: format!("{name}#TEST"),
    }
}
pub(super) fn sample(name: &str, number: u32) -> Sample {
    Sample {
        account: account(name),
        phase: "InProgress".into(),
        queue_active: true,
        observed_game: Some(ObservedGame {
            id: format!("HN1_{number}"),
            game_id: number.to_string(),
            champion_id: 103,
            queue_name: "排位".into(),
        }),
    }
}
pub(super) fn detail(name: &str, number: u32) -> Value {
    json!({"gameId":number,"platformId":"HN1","gameDuration":1200,"gameCreation":1000,"queueId":420,"participantIdentities":[{"participantId":1,"player":{"puuid":name}}],"participants":[{"participantId":1,"championId":103,"stats":{"win":true}}]})
}
pub(super) fn workspace(store: &Store) -> LiveWorkspace {
    store
        .workspace(ClientStatus::unavailable("offline", "test", 0))
        .unwrap()
}
#[test]
fn account_shortcuts_persist_without_claiming_play_and_validate_identity() {
    let path = std::env::temp_dir().join(format!("league-replay-shortcuts-{}.sqlite", id()));
    {
        let mut store = Store::new(Connection::open(&path).unwrap()).unwrap();
        // Archives written before shortcuts were introduced still load.
        store
            .0
            .execute(
                "UPDATE user_state SET data=?1 WHERE id=1",
                [r#"{"notes":[],"reviewed":[],"theme":null}"#],
            )
            .unwrap();
        let mut ui = workspace(&store).ui;
        assert!(ui.my_accounts.is_empty());
        ui.my_accounts.push(SavedAccount {
            server: "TENCENT_HN1".into(),
            account: account("a"),
        });
        store.save_ui(&ui, 0).unwrap();
        let mut invalid = ui.clone();
        invalid.my_accounts.push(invalid.my_accounts[0].clone());
        assert_eq!(store.save_ui(&invalid, 1).unwrap_err(), "accounts.invalid");
        invalid = ui.clone();
        invalid.my_accounts[0].server = "TENCENT_HN10".into();
        assert!(store.save_ui(&invalid, 1).is_err());
        invalid = ui;
        invalid.my_accounts[0].account.id = "HN1:someone-else".into();
        assert!(store.save_ui(&invalid, 1).is_err());
    }
    {
        let mut store = Store::new(Connection::open(&path).unwrap()).unwrap();
        let view = workspace(&store);
        assert_eq!(view.ui.my_accounts[0].account.id, "HN1:a");
        assert!(view.matches.is_empty() && view.sessions.is_empty() && view.accounts.is_empty());
        let subscriptions: u32 = store
            .0
            .query_row("SELECT COUNT(*) FROM subscriptions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(subscriptions, 0);
        store
            .import(&account("a"), &detail("a", 1), None, None, 5000)
            .unwrap();
        let mut ui = view.ui;
        ui.my_accounts.clear();
        store.save_ui(&ui, view.ui_revision).unwrap();
        assert_eq!(workspace(&store).matches.len(), 1);
    }
    std::fs::remove_file(path).unwrap();
}
#[test]
fn switch_reconnect_and_late_results_keep_frozen_ownership() {
    let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
    store.observe(Some(&sample("a", 1)), 1000).unwrap();
    store.observe(None, 2000).unwrap();
    store.observe(Some(&sample("a", 1)), 3000).unwrap();
    store.observe(Some(&sample("b", 2)), 4000).unwrap();
    let view = workspace(&store);
    assert_eq!(view.matches.len(), 2);
    assert_eq!(view.sessions.len(), 1);
    assert_eq!(view.sessions[0].segments.len(), 2);
    assert!(store
        .complete(
            "HN1_1",
            &account("a"),
            &detail("b", 1),
            Err("client.NotFound"),
            4000
        )
        .is_err());
    store
        .complete(
            "HN1_1",
            &account("a"),
            &detail("a", 1),
            Err("client.NotFound"),
            4000,
        )
        .unwrap();
    assert_eq!(
        workspace(&store)
            .matches
            .iter()
            .find(|m| m.id == "HN1_1")
            .unwrap()
            .account_id,
        "HN1:a"
    );
}

#[test]
fn post_game_recovery_tracks_borrowed_accounts_once_and_resumes_after_restart() {
    let path = std::env::temp_dir().join(format!("league-replay-recovery-{}.sqlite", id()));
    let mut finished = sample("a", 1);
    finished.phase = "EndOfGame".into();
    finished.queue_active = false;
    {
        let mut store = Store::new(Connection::open(&path).unwrap()).unwrap();
        // Start the app only after the first borrowed-account game has ended.
        store.observe(Some(&finished), 1000).unwrap();
        store.observe(Some(&finished), 2000).unwrap();
        assert_eq!(store.pending("HN1", 2000).unwrap().unwrap().0, "HN1_1");
        let mut second = sample("b", 2);
        second.phase = "Reconnect".into();
        store.observe(Some(&second), 3000).unwrap();
        second.phase = "WaitingForStats".into();
        store.observe(Some(&second), 4000).unwrap();
        store.observe(Some(&second), 5000).unwrap();
        let view = workspace(&store);
        assert_eq!(view.matches.len(), 2);
        assert_eq!(view.sessions.len(), 1);
        assert_eq!(view.sessions[0].segments.len(), 2);
        assert!(view
            .matches
            .iter()
            .all(|m| m.automatic && !m.manual && m.state == "pending"));
        assert!(view.ui.my_accounts.is_empty());
    }
    {
        let mut store = Store::new(Connection::open(&path).unwrap()).unwrap();
        let (game, _, account) = store.pending("HN1", 6000).unwrap().unwrap();
        assert_eq!(account.id, "HN1:a");
        store
            .complete(
                &game,
                &account,
                &detail("a", 1),
                Ok(&json!({"frames":[]})),
                6000,
            )
            .unwrap();
        assert_eq!(store.pending("HN1", 6000).unwrap().unwrap().0, "HN1_2");
        // An old end screen neither duplicates a record nor holds its session open forever.
        store.observe(Some(&finished), 5000 + IDLE_MS).unwrap();
        store.observe(Some(&finished), 6000 + IDLE_MS).unwrap();
        let view = workspace(&store);
        assert_eq!(view.matches.len(), 2);
        assert_eq!(view.sessions.len(), 1);
        assert!(view.sessions[0].ended_at.is_some());
        assert_eq!(
            view.matches.iter().find(|m| m.id == "HN1_1").unwrap().state,
            "ready"
        );
    }
    std::fs::remove_file(path).unwrap();
}
#[test]
fn idle_boundary_and_same_game_resume_are_distinct() {
    let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
    store.observe(Some(&sample("a", 1)), 1000).unwrap();
    store
        .observe(Some(&sample("a", 1)), 1000 + IDLE_MS)
        .unwrap();
    assert_eq!(workspace(&store).sessions.len(), 1);
    store
        .observe(Some(&sample("b", 2)), 1000 + 2 * IDLE_MS)
        .unwrap();
    let view = workspace(&store);
    assert_eq!(view.sessions.len(), 2);
    assert_eq!(
        view.sessions
            .iter()
            .filter(|s| s.ended_at.is_none())
            .count(),
        1
    );
}

#[test]
fn confirmed_settlement_retries_immediately_after_a_transient_disconnect() {
    let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
    let mut current = sample("a", 1);
    store.observe(Some(&current), 1000).unwrap();
    store.observe(None, 2000).unwrap();
    store.failed("HN1_1", 3000, "client.NotFound").unwrap();
    store.observe(Some(&current), 4000).unwrap();
    assert!(store.pending("HN1", 4000).unwrap().is_none());
    current.phase = "EndOfGame".into();
    current.queue_active = false;
    store.observe(Some(&current), 5000).unwrap();
    assert_eq!(
        store.pending("HN1", 5000).unwrap().map(|p| p.0).as_deref(),
        Some("HN1_1")
    );
    // Repeated end-screen samples must preserve the next genuine failure's backoff.
    store.failed("HN1_1", 6000, "client.NotFound").unwrap();
    store.observe(Some(&current), 7000).unwrap();
    assert!(store.pending("HN1", 7000).unwrap().is_none());
}
#[test]
fn manual_import_is_idempotent_and_does_not_impersonate_observation() {
    let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
    store
        .import(&account("a"), &detail("a", 1), None, None, 5000)
        .unwrap();
    store
        .import(&account("a"), &detail("a", 1), None, None, 6000)
        .unwrap();
    let view = workspace(&store);
    assert_eq!(view.matches.len(), 1);
    assert!(!view.matches[0].automatic);
    assert!(view.matches[0].manual);
    assert!(view.sessions[0].ended_at.is_some());
    store.observe(Some(&sample("a", 1)), 7000).unwrap();
    let view = workspace(&store);
    assert_eq!(view.matches.len(), 1);
    assert!(view.matches[0].automatic);
    store
        .retract_manual(&view.matches[0].observation_id)
        .unwrap();
    let view = workspace(&store);
    assert_eq!(view.matches.len(), 1);
    assert!(!view.matches[0].manual);
}
#[test]
fn ui_revision_and_notes_survive_fact_updates_and_retraction() {
    let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
    store
        .import(&account("a"), &detail("a", 1), None, None, 5000)
        .unwrap();
    let mut ui = LiveUiState::default();
    ui.notes.push(LiveNote {
        id: "note".into(),
        match_id: "HN1_1".into(),
        body: "记得控线".into(),
        at: None,
        participant_id: None,
        tags: vec![],
        updated_at: "2026-09-21".into(),
    });
    assert_eq!(store.save_ui(&ui, 0).unwrap(), 1);
    assert!(store.save_ui(&LiveUiState::default(), 0).is_err());
    store
        .complete(
            "HN1_1",
            &account("a"),
            &detail("a", 1),
            Err("client.NotFound"),
            4000,
        )
        .unwrap();
    let view = workspace(&store);
    store
        .retract_manual(&view.matches[0].observation_id)
        .unwrap();
    let view = workspace(&store);
    assert!(view.matches.is_empty());
    assert_eq!(view.ui.notes[0].body, "记得控线");
}
#[test]
fn player_aliases_are_explicit_and_cannot_overlap() {
    let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
    let mut ui = LiveUiState {
        players: vec![
            PlayerProfile {
                id: "1".into(),
                name: "XX".into(),
                account_ids: vec!["HN1:a".into(), "HN1:b".into()],
            },
            PlayerProfile {
                id: "2".into(),
                name: "YY".into(),
                account_ids: vec!["HN1:a".into()],
            },
        ],
        ..Default::default()
    };
    assert!(store.save_ui(&ui, 0).is_err());
    ui.players.pop();
    store.save_ui(&ui, 0).unwrap();
    assert_eq!(workspace(&store).ui.players[0].account_ids.len(), 2);
}

#[test]
fn persisted_identity_survives_restart_and_game_numbers_are_region_scoped() {
    let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
    store.observe(Some(&sample("a", 1)), 1000).unwrap();
    // Rebuild the observer store without retaining an in-memory active-session object.
    let mut store = Store::new(store.0).unwrap();
    store.observe(Some(&sample("a", 1)), 2000).unwrap();
    let mut overseas = sample("a", 1);
    overseas.account.platform = "EUW1".into();
    overseas.account.id = "EUW1:a".into();
    overseas.observed_game.as_mut().unwrap().id = "EUW1_1".into();
    store.observe(Some(&overseas), 3000).unwrap();
    let view = workspace(&store);
    assert_eq!(view.matches.len(), 2);
    assert_eq!(view.sessions.len(), 1);
    assert_eq!(view.sessions[0].segments.len(), 2);
}

#[test]
fn disk_reopen_preserves_play_archive_review_data_and_user_notes() {
    struct TestDatabase(std::path::PathBuf);
    impl Drop for TestDatabase {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let database = TestDatabase(std::env::temp_dir().join(format!(
        "league-replay-archive-{}.sqlite",
        uuid::Uuid::new_v4()
    )));
    let mut store = Store::new(Connection::open(&database.0).unwrap()).unwrap();
    store.observe(Some(&sample("a", 1)), 1000).unwrap();
    store.observe(Some(&sample("b", 2)), 2000).unwrap();
    let mut ui = LiveUiState::default();
    ui.notes.push(LiveNote {
        id: "own-note".into(),
        match_id: "HN1_1".into(),
        body: "自己的复盘".into(),
        at: Some(42.0),
        participant_id: Some(1),
        tags: vec!["对线".into()],
        updated_at: "2026-09-21".into(),
    });
    ui.players.push(PlayerProfile {
        id: "buddy".into(),
        name: "老朋友".into(),
        account_ids: vec!["HN1:b".into()],
    });
    store.save_ui(&ui, 0).unwrap();
    let original = serde_json::to_value(workspace(&store)).unwrap();
    drop(store);

    let mut store = Store::new(Connection::open(&database.0).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(workspace(&store)).unwrap(), original);
    let mut other_detail = detail("other", 3);
    other_detail["participantIdentities"][0]["player"] = json!({
        "puuid":"other", "summonerId":2, "gameName":"other", "tagLine":"TEST"
    });
    let review = crate::domain::review::StoredReview {
        id: "HN1_3".into(),
        server: "TENCENT_HN1".into(),
        account: account("other"),
        detail: other_detail,
        timeline: Some(json!({"gameId":3,"frames":[]})),
        source: "lcu".into(),
        timeline_error: None,
        bookmarked: false,
    };
    store.save_review(&review, 3000).unwrap();
    store.bookmark(&review.id, true).unwrap();
    store
        .save_replay(&review.id, 12345, "16.18.819.1953")
        .unwrap();
    ui.notes.push(LiveNote {
        id: "study-note".into(),
        match_id: review.id.clone(),
        body: "学习高手的处理".into(),
        at: Some(75.0),
        participant_id: Some(1),
        tags: vec!["学习".into()],
        updated_at: "2026-09-21".into(),
    });
    store.save_ui(&ui, 1).unwrap();
    drop(store);

    let store = Store::new(Connection::open(&database.0).unwrap()).unwrap();
    let restored = serde_json::to_value(workspace(&store)).unwrap();
    assert_eq!(restored["matches"], original["matches"]);
    assert_eq!(restored["sessions"], original["sessions"]);
    assert_eq!(restored["ui"]["players"], original["ui"]["players"]);
    assert_eq!(
        restored["ui"]["notes"],
        serde_json::to_value(&ui.notes).unwrap()
    );
    assert_eq!(restored["uiRevision"], 2);
    let saved = store
        .cached_review(&review.id, Some("other"))
        .unwrap()
        .unwrap();
    assert!(saved.bookmarked && saved.timeline.is_some());
    assert_eq!(saved.account.puuid, "other");
    assert_eq!(store.library(0, "downloaded", "").unwrap()[0].id, review.id);
    assert_eq!(store.replay_files().unwrap()[0].received, 12345.0);
}

#[test]
fn manual_import_rolls_back_bad_destination_and_other_players_remain_named() {
    let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
    let mut game = detail("a", 1);
    game["participantIdentities"].as_array_mut().unwrap().push(json!({"participantId":2,"player":{"puuid":"buddy","summonerId":3,"gameName":"好友","tagLine":"TEST"}}));
    assert!(store
        .import(&account("a"), &game, None, Some("missing"), 5000)
        .is_err());
    assert!(workspace(&store).matches.is_empty());
    assert!(workspace(&store).accounts.is_empty());
    store
        .import(&account("a"), &game, None, None, 5000)
        .unwrap();
    let view = workspace(&store);
    store
        .retract_manual(&view.matches[0].observation_id)
        .unwrap();
    assert_eq!(
        workspace(&store)
            .accounts
            .iter()
            .find(|a| a.puuid == "buddy")
            .unwrap()
            .riot_id,
        "好友#TEST"
    );
}
