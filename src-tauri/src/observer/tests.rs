use super::*;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::future::ready;

fn fixture() -> (std::path::PathBuf, SharedObserver) {
    let dir = std::env::temp_dir().join(format!("observer-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let store = archive::Store::new(Connection::open(dir.join("archive.sqlite")).unwrap()).unwrap();
    let observer = Observer::new(Arc::new(Mutex::new(store)), dir.join("collection.jsonl"));
    (dir, observer)
}

fn sample(name: &str, number: u32, phase: &str) -> Result<Sample, lcu::LcuError> {
    let account = json!({"puuid":name,"summonerId":1,"gameName":"PrivateName","tagLine":"PRIVATE"});
    let session = json!({"phase":phase,"gameClient":{"running":true},"gameData":{"gameId":number,"teamOne":[{"puuid":name,"championId":103}]}});
    super::sample::parse_sample("HN1", &account, &account, phase, Some(&session))
}

fn detail(name: &str, number: u32) -> Value {
    json!({"gameId":number,"platformId":"HN1","gameDuration":1200,"gameCreation":1000,"queueId":420,"participantIdentities":[{"participantId":1,"player":{"puuid":name,"summonerId":1,"gameName":"PrivateName","tagLine":"PRIVATE"}}],"participants":[{"participantId":1,"championId":103,"stats":{"win":true}}]})
}

#[tokio::test]
async fn offline_lifecycle_retries_restarts_and_keeps_account_ownership() {
    let (dir, observer) = fixture();
    let start = now();
    observer.record_sample(&Err(lcu::LcuError::Offline), start);
    observer.record_sample(&sample("private-a", 1, "Lobby"), start + 1);
    observer.record_sample(&sample("private-a", 1, "InProgress"), start + 2);
    observer.record_sample(&Err(lcu::LcuError::Unavailable), start + 3);
    observer.record_sample(&sample("private-a", 1, "Reconnect"), start + 4);
    observer.record_sample(&sample("private-a", 1, "EndOfGame"), start + 5);
    let a = sample("private-a", 1, "EndOfGame").unwrap().account;
    // Timeout/unavailable and data-not-ready follow the same production retry path.
    for error in ["client.Unavailable", "client.NotFound"] {
        observer
            .fetch_game("HN1_1", &a, ready(Err(error.into())), async {
                panic!("timeline must not be requested when detail fails")
            })
            .await;
        assert!(observer
            .store
            .lock()
            .unwrap()
            .pending("HN1", now())
            .unwrap()
            .is_none());
        assert!(observer
            .store
            .lock()
            .unwrap()
            .pending("HN1", now() + 60_001)
            .unwrap()
            .is_some());
    }
    // Invalid identity must not save detail or even request the timeline.
    observer
        .fetch_game("HN1_1", &a, ready(Ok(detail("someone-else", 1))), async {
            panic!("timeline must not be requested for an unverified account")
        })
        .await;
    observer.record_sample(&sample("private-b", 2, "InProgress"), start + 6);
    let malformed = crate::matches::lcu_timeline("TENCENT_HN1", "1", json!({"frames":{}}));
    assert!(malformed.is_err());
    observer
        .fetch_game(
            "HN1_1",
            &a,
            ready(Ok(detail("private-a", 1))),
            ready(malformed),
        )
        .await;
    {
        let store = observer.store.lock().unwrap();
        let game = store.cached_review("HN1_1", None).unwrap().unwrap();
        assert_eq!(game.account.id, a.id);
        assert!(game.timeline.is_none());
        assert_eq!(game.timeline_error.as_deref(), Some("game.invalidTimeline"));
    }
    drop(observer);
    let store = archive::Store::new(Connection::open(dir.join("archive.sqlite")).unwrap()).unwrap();
    assert!(store.pending("NA1", now() + 60_001).unwrap().is_none());
    let observer = Observer::new(Arc::new(Mutex::new(store)), dir.join("collection.jsonl"));
    observer.record_sample(&sample("private-b", 2, "EndOfGame"), start + 7);
    let timeline = crate::matches::lcu_timeline(
        "TENCENT_HN1",
        "1",
        json!({"frames":[{"timestamp":0,"events":[]}]}),
    )
    .unwrap();
    observer
        .fetch_game(
            "HN1_1",
            &a,
            ready(Ok(detail("private-a", 1))),
            ready(Ok(timeline.clone())),
        )
        .await;
    observer
        .fetch_game(
            "HN1_1",
            &a,
            ready(Err("client.Unavailable".into())),
            ready(Ok(Value::Null)),
        )
        .await;
    {
        let store = observer.store.lock().unwrap();
        let game = store.cached_review("HN1_1", None).unwrap().unwrap();
        assert_eq!(game.timeline, Some(timeline));
        assert_eq!(game.timeline_error, None);
        let view = store
            .workspace(observer.status.lock().unwrap().clone())
            .unwrap();
        assert_eq!(view.matches.len(), 2);
        assert_eq!(
            view.matches
                .iter()
                .find(|m| m.id == "HN1_1")
                .unwrap()
                .account_id,
            a.id
        );
    }
    let log = std::fs::read_to_string(dir.join("collection.jsonl")).unwrap();
    for event in [
        "fetch_started",
        "fetch_failed",
        "fetch_saved",
        "game.invalidTimeline",
    ] {
        assert!(log.contains(event));
    }
    for private in [
        "private-a",
        "private-b",
        "PrivateName",
        "PRIVATE",
        "participantIdentities",
    ] {
        assert!(!log.contains(private));
    }
    drop(observer);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn storage_and_log_failures_do_not_claim_success_or_block_recovery() {
    let (dir, observer) = fixture();
    observer.store.lock().unwrap().test_connection().execute_batch(
        "CREATE TRIGGER reject_observation BEFORE INSERT ON participations BEGIN SELECT RAISE(ABORT,'test write failure'); END;"
    ).unwrap();
    observer.record_sample(&sample("a", 1, "InProgress"), 1);
    assert_eq!(observer.status.lock().unwrap().state, "storage-error");
    assert!(observer
        .store
        .lock()
        .unwrap()
        .cached_review("HN1_1", None)
        .unwrap()
        .is_none());
    observer
        .store
        .lock()
        .unwrap()
        .test_connection()
        .execute_batch("DROP TRIGGER reject_observation;")
        .unwrap();
    observer.record_sample(&sample("a", 1, "InProgress"), 2);
    assert_eq!(observer.status.lock().unwrap().state, "connected");
    observer.store.lock().unwrap().test_connection().execute_batch(
        "CREATE TRIGGER reject_timeline BEFORE UPDATE OF timeline ON games BEGIN SELECT RAISE(ABORT,'test write failure'); END;"
    ).unwrap();
    let a = sample("a", 1, "EndOfGame").unwrap().account;
    observer
        .fetch_game(
            "HN1_1",
            &a,
            ready(Ok(detail("a", 1))),
            ready(Ok(json!({"frames":[]}))),
        )
        .await;
    assert!(observer
        .store
        .lock()
        .unwrap()
        .cached_review("HN1_1", None)
        .unwrap()
        .is_none());
    let log = std::fs::read_to_string(dir.join("collection.jsonl")).unwrap();
    assert!(log.contains("fetch_failed"));
    assert!(!log.contains("fetch_saved"));
    observer
        .store
        .lock()
        .unwrap()
        .test_connection()
        .execute_batch("DROP TRIGGER reject_timeline;")
        .unwrap();
    // An unusable log destination must not turn a successful archive write into a failure.
    *observer.diagnostics.lock().unwrap() = diagnostics::Diagnostics::new(dir.clone());
    observer.record_sample(&sample("a", 1, "EndOfGame"), 3);
    assert_eq!(observer.status.lock().unwrap().state, "connected");
    assert_eq!(
        observer
            .store
            .lock()
            .unwrap()
            .pending("HN1", 3)
            .unwrap()
            .unwrap()
            .0,
        "HN1_1"
    );
    observer
        .fetch_game(
            "HN1_1",
            &a,
            ready(Ok(detail("a", 1))),
            ready(Ok(json!({"frames":[]}))),
        )
        .await;
    assert!(observer
        .store
        .lock()
        .unwrap()
        .cached_review("HN1_1", None)
        .unwrap()
        .unwrap()
        .timeline
        .is_some());
    drop(observer);
    std::fs::remove_dir_all(dir).unwrap();
}
