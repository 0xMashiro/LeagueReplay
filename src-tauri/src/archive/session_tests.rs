use super::{tests::*, *};
use serde_json::json;

#[test]
fn batched_membership_uses_game_time_and_preserves_segment_ties_and_empty_sessions() {
    let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
    for (n, name) in [(1, "a"), (2, "a"), (3, "b"), (4, "a")] {
        store
            .observe(Some(&sample(name, n)), i64::from(n) * 100)
            .unwrap();
    }
    for (n, name, started) in [
        (1, "a", 3000),
        (2, "a", 2500),
        (3, "b", 1000),
        (4, "a", 1000),
    ] {
        store
            .complete(
                &format!("HN1_{n}"),
                &account(name),
                &at(name, n, started),
                Err("client.NotFound"),
                started,
            )
            .unwrap();
    }
    store
        .0
        .execute(
            "INSERT INTO sessions VALUES('empty','Empty',0,0,0,'import')",
            [],
        )
        .unwrap();
    store
        .0
        .execute("INSERT INTO segments VALUES('unused','empty','HN1:a')", [])
        .unwrap();
    let view = workspace(&store);
    assert_eq!(view.sessions.len(), 2);
    assert!(view.sessions[1].segments.is_empty());
    let session = &view.sessions[0];
    let expected = sessions::ordered_segments(&store.0, &session.id).unwrap();
    assert_eq!(
        session.segments.iter().map(|s| &s.id).collect::<Vec<_>>(),
        expected.iter().map(|s| &s.id).collect::<Vec<_>>()
    );
    let game_ids: Vec<_> = session
        .segments
        .iter()
        .flat_map(|s| &s.match_ids)
        .map(|id| {
            view.matches
                .iter()
                .find(|record| &record.observation_id == id)
                .unwrap()
                .id
                .as_str()
        })
        .collect();
    assert_eq!(game_ids, ["HN1_3", "HN1_4", "HN1_2", "HN1_1"]);
}

#[test]
fn batch_import_is_one_session_and_rolls_back_the_entire_invalid_batch() {
    use crate::domain::review::StoredReview;
    let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
    let make = |n| StoredReview {
        id: format!("HN1_{n}"),
        server: "TENCENT_HN1".into(),
        account: account("a"),
        detail: detail("a", n),
        timeline: None,
        source: "archive".into(),
        timeline_error: None,
        bookmarked: false,
    };
    let mut invalid = make(2);
    invalid.detail["gameId"] = json!(0);
    assert!(store
        .import_many(&[make(1), invalid], None, "Batch")
        .is_err());
    assert!(workspace(&store).matches.is_empty());
    assert!(workspace(&store).sessions.is_empty());
    store
        .import_many(&[make(1), make(2)], None, "Batch")
        .unwrap();
    let view = workspace(&store);
    assert_eq!(view.sessions.len(), 1);
    assert_eq!(view.matches.len(), 2);
    assert!(view.matches.iter().all(|m| m.manual && !m.automatic));
}

#[test]
fn arbitrary_matches_move_atomically_without_changing_identity_or_recording_clock() {
    let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
    for n in 1..=3 {
        store
            .observe(Some(&sample("a", n)), i64::from(n) * 2000)
            .unwrap();
        store
            .complete(
                &format!("HN1_{n}"),
                &account("a"),
                &at("a", n, i64::from(n) * 2000),
                Err("client.NotFound"),
                i64::from(n) * 2000,
            )
            .unwrap();
    }
    let before = workspace(&store);
    let middle = &before.matches[1];
    let target = store
        .move_matches(
            std::slice::from_ref(&middle.observation_id),
            None,
            "中间一局",
        )
        .unwrap();
    let after = workspace(&store);
    assert_eq!(after.sessions.len(), 2);
    assert_eq!(after.matches.len(), 3);
    let moved = after.matches.iter().find(|m| m.id == middle.id).unwrap();
    assert_eq!(moved.account_id, middle.account_id);
    assert!(moved.automatic && !moved.manual);
    assert_eq!(
        after
            .sessions
            .iter()
            .find(|s| s.id == target)
            .unwrap()
            .segments[0]
            .match_ids,
        vec![middle.observation_id.clone()]
    );
    let snapshot = serde_json::to_value(&after).unwrap();
    assert!(store
        .move_matches(
            &[middle.observation_id.clone(), "missing".into()],
            Some(&before.sessions[0].id),
            ""
        )
        .is_err());
    assert_eq!(serde_json::to_value(workspace(&store)).unwrap(), snapshot);
    store.observe(None, 6000 + IDLE_MS).unwrap();
    assert_eq!(
        workspace(&store)
            .sessions
            .iter()
            .find(|s| s.id == before.sessions[0].id)
            .unwrap()
            .ended_at,
        Some((6000 + IDLE_MS) as f64)
    );
}

fn at(name: &str, number: u32, start: i64) -> Value {
    let mut game = detail(name, number);
    game["gameCreation"] = json!(start);
    game
}

#[test]
fn session_edits_preserve_facts_marks_and_reconnect_after_reopening() {
    let db = std::env::temp_dir().join(format!("league-replay-sessions-{}.sqlite", id()));
    let mut store = Store::new(Connection::open(&db).unwrap()).unwrap();
    store.observe(Some(&sample("a", 1)), 1000).unwrap();
    store
        .complete(
            "HN1_1",
            &account("a"),
            &at("a", 1, 1000),
            Ok(&json!({"frames":[]})),
            1000,
        )
        .unwrap();
    store.observe(Some(&sample("b", 2)), 1_501_000).unwrap();
    store
        .complete(
            "HN1_2",
            &account("b"),
            &at("b", 2, 1_501_000),
            Err("client.NotFound"),
            1_501_000,
        )
        .unwrap();
    let session = workspace(&store).sessions[0].clone();
    store
        .import(
            &account("a"),
            &at("a", 1, 1000),
            None,
            Some(&session.id),
            1_600_000,
        )
        .unwrap();
    let ui = LiveUiState {
        notes: vec![LiveNote {
            id: "note".into(),
            match_id: "HN1_1".into(),
            body: "控线".into(),
            at: Some(30.0),
            participant_id: Some(1),
            tags: vec!["对线".into()],
            updated_at: "today".into(),
        }],
        players: vec![PlayerProfile {
            id: "friend".into(),
            name: "朋友".into(),
            account_ids: vec!["HN1:b".into()],
        }],
        premades: vec![PremadeMark {
            match_id: "HN1_1".into(),
            account_id: "HN1:b".into(),
        }],
        ..Default::default()
    };
    store.save_ui(&ui, 0).unwrap();
    let before = serde_json::to_value(workspace(&store)).unwrap();
    store.rename_session(&session.id, "  晚间双排  ").unwrap();
    store
        .split_session(&session.id, &session.segments[1].id, "切号后")
        .unwrap();
    let split = workspace(&store);
    assert_eq!(split.sessions.len(), 2);
    let tail = split
        .sessions
        .iter()
        .find(|s| s.ended_at.is_none())
        .unwrap();
    assert_eq!(tail.title, "切号后");
    assert_eq!(tail.segments[0].id, session.segments[1].id);
    // Transfer the open session into the named, closed destination.
    store.merge_sessions(&tail.id, &session.id).unwrap();
    let merged = serde_json::to_value(workspace(&store)).unwrap();
    assert_eq!(merged["matches"], before["matches"]);
    assert_eq!(merged["ui"], before["ui"]);
    assert_eq!(merged["uiRevision"], 1);
    assert_eq!(merged["sessions"][0]["title"], "晚间双排");
    assert!(merged["sessions"][0]["endedAt"].is_null());
    drop(store);
    let mut store = Store::new(Connection::open(&db).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(workspace(&store)).unwrap(), merged);
    store.observe(None, 1_502_000).unwrap();
    store.observe(Some(&sample("b", 2)), 1_503_000).unwrap();
    store.observe(Some(&sample("b", 3)), 1_504_000).unwrap();
    let resumed = workspace(&store);
    assert_eq!(resumed.sessions.len(), 1);
    assert_eq!(resumed.matches.len(), 3);
    assert_eq!(
        resumed
            .matches
            .iter()
            .find(|m| m.id == "HN1_3")
            .unwrap()
            .segment_id,
        session.segments[1].id
    );
    store.observe(None, 1_504_000 + IDLE_MS).unwrap();
    assert_eq!(
        workspace(&store).sessions[0].ended_at,
        Some((1_504_000 + IDLE_MS) as f64)
    );
    drop(store);
    std::fs::remove_file(db).unwrap();
}

#[test]
fn historic_backfill_and_merge_do_not_redirect_new_play_or_extend_idle_clock() {
    let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
    store.observe(Some(&sample("a", 1)), 10_000_000).unwrap();
    store.observe(Some(&sample("b", 2)), 10_500_000).unwrap();
    let session = workspace(&store).sessions[0].clone();
    let current_segment = session.segments[1].id.clone();
    store
        .import(
            &account("a"),
            &at("a", 3, 1000),
            None,
            Some(&session.id),
            10_600_000,
        )
        .unwrap();
    store.observe(Some(&sample("b", 4)), 10_700_000).unwrap();
    assert_eq!(
        workspace(&store)
            .matches
            .iter()
            .find(|m| m.id == "HN1_4")
            .unwrap()
            .segment_id,
        current_segment
    );
    assert_eq!(workspace(&store).sessions[0].segments.len(), 3);
    // A closed session can have a later activity timestamp than the open one.
    store
        .import(
            &account("c"),
            &at("c", 5, 11_000_000),
            None,
            None,
            11_100_000,
        )
        .unwrap();
    let closed = workspace(&store)
        .sessions
        .into_iter()
        .find(|s| s.ended_at.is_some())
        .unwrap();
    store.merge_sessions(&closed.id, &session.id).unwrap();
    store.observe(None, 10_700_000 + IDLE_MS).unwrap();
    assert_eq!(
        workspace(&store).sessions[0].ended_at,
        Some((10_700_000 + IDLE_MS) as f64)
    );
}

#[test]
fn invalid_edits_rollback_and_stale_attribution_cannot_move_a_match() {
    let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
    store.observe(Some(&sample("a", 1)), 1000).unwrap();
    let session = workspace(&store).sessions[0].clone();
    let before = serde_json::to_value(workspace(&store)).unwrap();
    for title in [" ".into(), "x".repeat(81), "a\nb".into()] {
        assert!(store.rename_session(&session.id, &title).is_err());
    }
    assert!(store.rename_session("missing", "有效").is_err());
    assert!(store.merge_sessions(&session.id, &session.id).is_err());
    assert!(store.merge_sessions(&session.id, "missing").is_err());
    assert!(store
        .split_session(&session.id, &session.segments[0].id, "尾场")
        .is_err());
    assert!(store.split_session(&session.id, "missing", "尾场").is_err());
    assert!(store
        .import(
            &account("a"),
            &at("a", 1, 1000),
            None,
            Some("missing"),
            2000
        )
        .is_err());
    assert_eq!(serde_json::to_value(workspace(&store)).unwrap(), before);
    store
        .import(
            &account("b"),
            &at("b", 2, 2000),
            None,
            Some(&session.id),
            3000,
        )
        .unwrap();
    let fresh = workspace(&store).sessions[0].clone();
    assert_eq!(
        store
            .split_session(&session.id, &fresh.segments[1].id, "尾场")
            .unwrap_err(),
        "session.activePrefix"
    );
    assert_eq!(workspace(&store).sessions.len(), 1);
}

#[test]
fn closed_sessions_split_and_backfill_use_real_game_times() {
    let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
    store
        .import(
            &account("a"),
            &at("a", 1, 2_000_000),
            None,
            None,
            20_000_000,
        )
        .unwrap();
    let session = workspace(&store).sessions[0].clone();
    store
        .import(
            &account("b"),
            &at("b", 2, 4_000_000),
            None,
            Some(&session.id),
            20_000_001,
        )
        .unwrap();
    let view = workspace(&store);
    assert_eq!(view.sessions[0].ended_at, Some(5_200_000.0));
    store
        .split_session(&session.id, &view.sessions[0].segments[1].id, "后半场")
        .unwrap();
    let view = workspace(&store);
    assert!(view.sessions.iter().all(|s| s.ended_at.is_some()));
    let original = view.sessions.iter().find(|s| s.id == session.id).unwrap();
    assert_eq!(original.started_at, 2_000_000.0);
    assert_eq!(original.ended_at, Some(3_200_000.0));
    let tail = view.sessions.iter().find(|s| s.id != session.id).unwrap();
    assert_eq!(tail.started_at, 4_000_000.0);
    store.merge_sessions(&tail.id, &original.id).unwrap();
    assert_eq!(workspace(&store).sessions[0].ended_at, Some(5_200_000.0));
}
