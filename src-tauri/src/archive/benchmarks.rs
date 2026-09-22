use super::*;
use std::time::Instant;

fn fixture(count: usize) -> Store {
    let mut store = Store::new(Connection::open("").unwrap()).unwrap();
    let tx = store.0.transaction().unwrap();
    tx.execute(
        "INSERT INTO accounts VALUES('HN1:a','HN1','a','1','a#TEST')",
        [],
    )
    .unwrap();
    let timeline =
        serde_json::json!({"frames": [], "fixturePadding": "x".repeat(4096)}).to_string();
    for n in 0..count {
        let session = format!("session-{}", n / 10);
        let segment = format!("segment-{}", n / 5);
        let game = format!("HN1_{}", n + 1);
        let at = 1_700_000_000_000_i64 + n as i64 * 1_800_000;
        if n % 10 == 0 {
            tx.execute(
                "INSERT INTO sessions VALUES(?1,'Benchmark',?2,?2,?2,'import')",
                params![session, at],
            )
            .unwrap();
        }
        if n % 5 == 0 {
            tx.execute(
                "INSERT INTO segments VALUES(?1,?2,'HN1:a')",
                params![segment, session],
            )
            .unwrap();
        }
        let mut detail = tests::detail("a", n as u32 + 1);
        detail["gameCreation"] = at.into();
        tx.execute(
            "INSERT INTO games(id,platform,game_number,detail,timeline) VALUES(?1,'HN1',?2,?3,?4)",
            params![game, (n + 1).to_string(), detail.to_string(), timeline],
        )
        .unwrap();
        tx.execute(
            "INSERT INTO participations VALUES(?1,?1,'HN1:a',?2,?3,?3,103,'420','ready',1,0)",
            params![game, segment, at],
        )
        .unwrap();
    }
    tx.commit().unwrap();
    store
}

#[test]
#[ignore = "offline synthetic archive benchmark; run explicitly with --ignored --nocapture"]
fn workspace_scale() {
    for count in [1_000, 10_000] {
        let store = fixture(count);
        let mut initial = Vec::new();
        let mut delta = Vec::new();
        let mut pages = Vec::new();
        let mut page_bytes = 0;
        let mut bytes = (0, 0);
        let cold = Instant::now();
        let cold_page = store.play_page(&PlayQuery::default()).unwrap();
        let cold_bytes = serde_json::to_vec(&cold_page).unwrap().len();
        println!(
            "matches={count} cold_page_ms={:.2} cold_page_bytes={cold_bytes}",
            cold.elapsed().as_secs_f64() * 1000.0
        );
        for _ in 0..5 {
            let status = || ClientStatus::unavailable("offline", "benchmark", 0);
            let start = Instant::now();
            let full = store.sync_workspace(status(), None).unwrap();
            let payload = serde_json::to_vec(&full).unwrap();
            initial.push(start.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(full.workspace.as_ref().unwrap().matches.len(), count);
            assert_eq!(full.workspace.as_ref().unwrap().sessions.len(), count / 10);
            bytes.0 = payload.len();
            let start = Instant::now();
            let unchanged = store.sync_workspace(status(), Some(full.cursor)).unwrap();
            let payload = serde_json::to_vec(&unchanged).unwrap();
            delta.push(start.elapsed().as_secs_f64() * 1000.0);
            assert!(unchanged.workspace.is_none());
            bytes.1 = payload.len();
            let start = Instant::now();
            let page = store.play_page(&PlayQuery::default()).unwrap();
            let payload = serde_json::to_vec(&page).unwrap();
            pages.push(start.elapsed().as_secs_f64() * 1000.0);
            page_bytes = payload.len();
            assert_eq!(page.sessions.len(), 20);
            assert_eq!(page.matches.len(), 200);
        }
        initial.sort_by(f64::total_cmp);
        delta.sort_by(f64::total_cmp);
        pages.sort_by(f64::total_cmp);
        println!(
            "matches={count} cached_summary_page_median_ms={:.2} page_bytes={page_bytes}",
            pages[2]
        );
        println!("matches={count} initial_median_ms={:.2} unchanged_median_ms={:.2} initial_bytes={} unchanged_bytes={}", initial[2], delta[2], bytes.0, bytes.1);
    }
}
