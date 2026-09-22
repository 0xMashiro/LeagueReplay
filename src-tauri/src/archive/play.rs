use super::*;

const PAGE_SIZE: u32 = 20;
// Resolve the account's participant once before filtering. Missing results stay NULL,
// so pending and incomplete matches never become losses.
const BASE: &str = "WITH facts AS (
    SELECT p.id,p.game_id,p.account_id,p.segment_id,p.first_seen,p.state,p.automatic,p.manual,
        seg.session_id,json_extract(g.detail,'$.gameCreation') AS created,
        (SELECT CASE WHEN json_type(person.value,'$.stats.win') IN ('true','false') THEN json_extract(person.value,'$.stats.win') END FROM json_each(g.detail,'$.participants') person
         JOIN json_each(g.detail,'$.participantIdentities') identity
           ON json_extract(identity.value,'$.participantId')=json_extract(person.value,'$.participantId')
         WHERE json_extract(identity.value,'$.player.puuid')=a.puuid LIMIT 1) AS result
    FROM participations p JOIN games g ON g.id=p.game_id
    JOIN accounts a ON a.id=p.account_id JOIN segments seg ON seg.id=p.segment_id
), results AS (
    SELECT *, CASE WHEN state='ready' THEN result END AS win FROM facts
), base AS (
    SELECT *, CASE WHEN win IS NOT NULL THEN COALESCE(NULLIF(created,0),first_seen)
        ELSE first_seen END AS started FROM results
)";
const FILTERED: &str = ", filtered AS (
    SELECT base.id,base.session_id FROM base
    JOIN games filter_game ON filter_game.id=base.game_id
    JOIN accounts filter_account ON filter_account.id=base.account_id
    JOIN sessions filter_session ON filter_session.id=base.session_id
    JOIN participations filter_record ON filter_record.id=base.id
    WHERE (?2 IS NULL OR base.account_id=?2) AND (?3 IS NULL OR win=?3)
    AND (?4='all' OR (?4='automatic' AND base.automatic=1) OR (?4='manual' AND base.manual=1))
    AND (?5 IS NULL OR started>=?5) AND (?6 IS NULL OR started<=?6)
    AND (?7='' OR play_text_match(base.game_id,filter_record.queue_name,filter_account.riot_id,filter_session.title,filter_record.champion_id))
    AND (?8 IS NULL OR play_player_match(filter_game.detail,filter_game.platform,filter_account.puuid,base.game_id,started))
)";
// Open sessions remain visible even when every game is excluded by filters.
const ELIGIBLE: &str = "(?1='all' OR (?1='current')=(s.ended_at IS NULL)) AND
    (s.ended_at IS NULL OR s.id IN (SELECT session_id FROM filtered))";

impl Store {
    fn play_summary(&self) -> Result<PlaySummary> {
        let revision: i64 = self
            .0
            .query_row("SELECT value FROM workspace_clock WHERE id=1", [], |r| {
                r.get(0)
            })
            .map_err(sql)?;
        // Never cache uncommitted state: a rollback can reuse the same revision later.
        let cacheable = self.0.is_autocommit();
        if cacheable {
            if let Some((cached_revision, summary)) = self.1.borrow().as_ref() {
                if *cached_revision == revision {
                    return Ok(summary.clone());
                }
            }
        }
        let summary = self.compute_play_summary()?;
        if cacheable {
            *self.1.borrow_mut() = Some((revision, summary.clone()));
        }
        Ok(summary)
    }

    fn compute_play_summary(&self) -> Result<PlaySummary> {
        let (settled, wins) = self
            .0
            .query_row(
                &format!(
                    "{BASE}, ranked AS (
            SELECT win,ROW_NUMBER() OVER(PARTITION BY game_id ORDER BY
                (SELECT rowid FROM participations WHERE id=base.id) DESC) AS position
            FROM base WHERE win IS NOT NULL
        ) SELECT COUNT(*),COALESCE(SUM(win),0) FROM ranked WHERE position=1"
                ),
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(sql)?;
        let (sessions,games,note_count) = self.0.query_row("SELECT
            (SELECT COUNT(*) FROM sessions),
            (SELECT COUNT(DISTINCT game_id) FROM participations),
            (SELECT COUNT(*) FROM user_state u,json_each(u.data,'$.notes') n
             WHERE u.id=1 AND json_extract(n.value,'$.matchId') IN (SELECT game_id FROM participations))",
            [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(sql)?;
        let mut stmt = self
            .0
            .prepare("SELECT DISTINCT account_id FROM participations ORDER BY account_id")
            .map_err(sql)?;
        let account_ids = stmt
            .query_map([], |r| r.get(0))
            .map_err(sql)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sql)?;
        Ok(PlaySummary {
            sessions,
            games,
            account_ids,
            settled,
            wins,
            note_count,
        })
    }

    pub fn play_page(&self, query: &PlayQuery) -> Result<PlayPage> {
        if query.from.is_some_and(|v| !v.is_finite()) || query.to.is_some_and(|v| !v.is_finite()) {
            return Err("library.invalidData".into());
        }
        play_filter::configure(&self.0, query)?;
        let scope = match query.scope {
            SessionScope::All => "all",
            SessionScope::Current => "current",
            SessionScope::History => "history",
        };
        let source = match query.source {
            PlaySource::All => "all",
            PlaySource::Automatic => "automatic",
            PlaySource::Manual => "manual",
        };
        let filters = params![
            scope,
            query.account,
            query.win,
            source,
            query.from,
            query.to,
            query.text.trim(),
            query.player
        ];
        let prefix = format!("{BASE}{FILTERED}");
        let total_sessions: u32 = self
            .0
            .query_row(
                &format!("{prefix} SELECT COUNT(*) FROM sessions s WHERE {ELIGIBLE}"),
                filters,
                |r| r.get(0),
            )
            .map_err(sql)?;
        let page = query.page.min(total_sessions.saturating_sub(1) / PAGE_SIZE);
        let mut stmt = self.0.prepare(&format!("{prefix} SELECT s.id,s.title,s.started_at,s.ended_at,s.end_reason
            FROM sessions s WHERE {ELIGIBLE} ORDER BY s.started_at DESC,s.rowid DESC LIMIT ?9 OFFSET ?10")).map_err(sql)?;
        let mut sessions = stmt
            .query_map(
                params![
                    scope,
                    query.account,
                    query.win,
                    source,
                    query.from,
                    query.to,
                    query.text.trim(),
                    query.player,
                    PAGE_SIZE,
                    i64::from(page) * i64::from(PAGE_SIZE)
                ],
                |r| {
                    Ok(PlaySession {
                        id: r.get(0)?,
                        title: r.get(1)?,
                        started_at: r.get::<_, i64>(2)? as f64,
                        ended_at: r.get::<_, Option<i64>>(3)?.map(|v| v as f64),
                        end_reason: r.get(4)?,
                        segments: Vec::new(),
                    })
                },
            )
            .map_err(sql)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sql)?;
        sessions::fill_segments(&self.0, &mut sessions)?;
        let ids = serde_json::to_string(&sessions.iter().map(|s| &s.id).collect::<Vec<_>>())
            .map_err(|_| "library.storageError")?;
        let mut stmt = self.0.prepare(&format!("{prefix}
            SELECT g.id,g.platform,g.game_number,p.account_id,p.segment_id,p.first_seen,p.last_seen,p.champion_id,p.queue_name,p.state,g.detail,g.data_error,p.id,p.automatic,p.manual
            FROM participations p JOIN games g ON g.id=p.game_id JOIN filtered f ON f.id=p.id
            WHERE f.session_id IN (SELECT value FROM json_each(?9)) AND ?1 IS NOT NULL
            ORDER BY COALESCE(json_extract(g.detail,'$.gameCreation'),p.first_seen),p.rowid")).map_err(sql)?;
        let matches = stmt
            .query_map(
                params![
                    scope,
                    query.account,
                    query.win,
                    source,
                    query.from,
                    query.to,
                    query.text.trim(),
                    query.player,
                    ids
                ],
                observed::read,
            )
            .map_err(sql)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sql)?;
        Ok(PlayPage {
            page,
            total_sessions,
            summary: self.play_summary()?,
            sessions,
            matches,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive() -> Store {
        let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
        for n in 1..=25 {
            let game = crate::domain::review::StoredReview {
                id: format!("HN1_{n}"),
                server: "TENCENT_HN1".into(),
                account: crate::archive::tests::account("a"),
                detail: crate::archive::tests::detail("a", n),
                timeline: None,
                source: "archive".into(),
                timeline_error: None,
                bookmarked: false,
            };
            store
                .import_many(&[game], None, &format!("Session {n}"))
                .unwrap();
        }
        // All sessions deliberately have the same time: rowid resolves ties.
        store
            .0
            .execute("UPDATE sessions SET started_at=1000", [])
            .unwrap();
        store
    }

    #[test]
    fn summary_cache_tracks_commits_without_retaining_rolled_back_values() {
        let store = archive();
        assert!(store.1.borrow().is_none());
        assert_eq!(store.play_summary().unwrap().games, 25);
        let revision = store.1.borrow().as_ref().unwrap().0;
        // Read-only paging/filtering does not change the summary version.
        store
            .play_page(&PlayQuery {
                page: 1,
                ..PlayQuery::default()
            })
            .unwrap();
        assert_eq!(store.1.borrow().as_ref().unwrap().0, revision);
        store
            .0
            .execute_batch("BEGIN; DELETE FROM participations WHERE game_id='HN1_1';")
            .unwrap();
        assert_eq!(store.play_summary().unwrap().games, 24);
        assert_eq!(store.1.borrow().as_ref().unwrap().0, revision);
        store.0.execute_batch("ROLLBACK;").unwrap();
        assert_eq!(store.play_summary().unwrap().games, 25);
        // Reuse the rolled-back revision for an unrelated commit.
        store
            .0
            .execute(
                "INSERT INTO sessions VALUES('empty','Empty',0,0,0,'import')",
                [],
            )
            .unwrap();
        let summary = store.play_summary().unwrap();
        assert_eq!((summary.games, summary.sessions), (25, 26));
        assert!(store.1.borrow().as_ref().unwrap().0 > revision);
        store
            .0
            .execute("DELETE FROM participations WHERE game_id='HN1_1'", [])
            .unwrap();
        assert_eq!(store.play_summary().unwrap().games, 24);
    }

    #[test]
    fn filters_before_paging_and_keeps_global_statistics() {
        let store = archive();
        store.0.execute("UPDATE games SET detail=json_set(detail,'$.participants[0].stats.win',json('false')) WHERE id IN ('HN1_2','HN1_22')", []).unwrap();
        store
            .0
            .execute(
                "UPDATE participations SET state='pending',first_seen=5000 WHERE game_id='HN1_1'",
                [],
            )
            .unwrap();
        store
            .0
            .execute(
                "UPDATE participations SET automatic=1,manual=0 WHERE game_id='HN1_2'",
                [],
            )
            .unwrap();
        store.0.execute("UPDATE user_state SET data=json_set(data,'$.notes',json(?1))", [r#"[{"matchId":"HN1_2","body":"one","id":"n1","tags":[],"updatedAt":"2026-09-22"},{"matchId":"HN1_2","body":"two","id":"n2","tags":[],"updatedAt":"2026-09-22"},{"matchId":"HN1_unknown","body":"study","id":"n3","tags":[],"updatedAt":"2026-09-22"}]"#]).unwrap();
        let query = PlayQuery {
            win: Some(false),
            ..PlayQuery::default()
        };
        let losses = store.play_page(&query).unwrap();
        assert_eq!(losses.total_sessions, 2);
        assert_eq!(losses.sessions[0].title, "Session 22");
        assert_eq!(losses.sessions[1].title, "Session 2");
        assert_eq!(losses.matches.len(), 2);
        assert_eq!(
            (
                losses.summary.sessions,
                losses.summary.games,
                losses.summary.settled,
                losses.summary.wins,
                losses.summary.note_count
            ),
            (25, 25, 24, 22, 2)
        );
        assert_eq!(losses.summary.account_ids, vec!["HN1:a"]);
        let automatic = store
            .play_page(&PlayQuery {
                source: PlaySource::Automatic,
                ..query.clone()
            })
            .unwrap();
        assert_eq!(automatic.matches[0].id, "HN1_2");
        assert_eq!(automatic.matches.len(), 1);
        let manual = store
            .play_page(&PlayQuery {
                source: PlaySource::Manual,
                ..query.clone()
            })
            .unwrap();
        assert_eq!(manual.matches[0].id, "HN1_22");
        assert_eq!(manual.matches.len(), 1);
        let missing = store
            .play_page(&PlayQuery {
                account: Some("HN1:missing".into()),
                ..query
            })
            .unwrap();
        assert!(missing.matches.is_empty());
        assert_eq!(missing.summary.games, 25);
        let date = store
            .play_page(&PlayQuery {
                from: Some(5000.0),
                to: Some(5000.0),
                ..PlayQuery::default()
            })
            .unwrap();
        assert_eq!(date.matches.len(), 1);
        assert_eq!(date.matches[0].id, "HN1_1");
        let before = store
            .play_page(&PlayQuery {
                to: Some(999.0),
                ..PlayQuery::default()
            })
            .unwrap();
        assert!(before.matches.is_empty());
        assert!(store
            .play_page(&PlayQuery {
                from: Some(f64::NAN),
                ..PlayQuery::default()
            })
            .is_err());
    }

    #[test]
    fn summary_counts_a_game_once_across_account_perspectives() {
        let mut store = archive();
        let mut game = crate::archive::tests::detail("a", 1);
        game["participantIdentities"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"participantId":2,"player":{"puuid":"b"}}));
        game["participants"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"participantId":2,"stats":{"win":false}}));
        store
            .import_many(
                &[crate::domain::review::StoredReview {
                    id: "HN1_1".into(),
                    server: "TENCENT_HN1".into(),
                    account: crate::archive::tests::account("b"),
                    detail: game,
                    timeline: None,
                    source: "archive".into(),
                    timeline_error: None,
                    bookmarked: false,
                }],
                None,
                "Second perspective",
            )
            .unwrap();
        let page = store
            .play_page(&PlayQuery {
                account: Some("HN1:b".into()),
                ..PlayQuery::default()
            })
            .unwrap();
        assert_eq!(page.matches.len(), 1);
        assert_eq!(page.summary.games, 25);
        assert_eq!(page.summary.settled, 25);
        assert_eq!(page.summary.wins, 24);
        assert_eq!(page.summary.account_ids, vec!["HN1:a", "HN1:b"]);
    }

    #[test]
    fn pages_have_stable_boundaries_and_clamp_stale_page_numbers() {
        let store = archive();
        let first = store
            .play_page(&PlayQuery {
                page: 0,
                scope: SessionScope::All,
                ..PlayQuery::default()
            })
            .unwrap();
        let last = store
            .play_page(&PlayQuery {
                page: u32::MAX,
                scope: SessionScope::All,
                ..PlayQuery::default()
            })
            .unwrap();
        assert_eq!(
            (
                first.page,
                first.total_sessions,
                first.sessions.len(),
                first.matches.len()
            ),
            (0, 25, 20, 20)
        );
        assert_eq!(
            (
                last.page,
                last.total_sessions,
                last.sessions.len(),
                last.matches.len()
            ),
            (1, 25, 5, 5)
        );
        assert_eq!(first.sessions[0].title, "Session 25");
        assert_eq!(last.sessions[0].title, "Session 5");
        let ids: std::collections::HashSet<_> = first
            .sessions
            .iter()
            .chain(&last.sessions)
            .map(|s| &s.id)
            .collect();
        assert_eq!(ids.len(), 25);
        for page in [&first, &last] {
            let members: std::collections::HashSet<_> = page
                .sessions
                .iter()
                .flat_map(|s| &s.segments)
                .flat_map(|s| &s.match_ids)
                .collect();
            assert_eq!(members.len(), page.matches.len());
            assert!(page
                .matches
                .iter()
                .all(|m| members.contains(&m.observation_id)));
        }
        store
            .0
            .execute("DELETE FROM participations WHERE game_id LIKE 'HN1_%'", [])
            .unwrap();
        let empty = store
            .play_page(&PlayQuery {
                page: 1,
                scope: SessionScope::History,
                ..PlayQuery::default()
            })
            .unwrap();
        assert_eq!((empty.page, empty.total_sessions), (0, 0));
        assert!(empty.sessions.is_empty() && empty.matches.is_empty());
    }

    #[test]
    fn scope_preserves_open_empty_sessions_and_only_projects_selected_page() {
        let store = archive();
        store
            .0
            .execute(
                "INSERT INTO sessions VALUES('open','Open',2000,2000,NULL,NULL)",
                [],
            )
            .unwrap();
        let current = store
            .play_page(&PlayQuery {
                page: 0,
                scope: SessionScope::Current,
                ..PlayQuery::default()
            })
            .unwrap();
        assert_eq!(current.total_sessions, 1);
        assert_eq!(current.sessions[0].id, "open");
        assert!(current.matches.is_empty());
        // A malformed summary on another page must not be projected or fail this page.
        store
            .0
            .execute("UPDATE games SET detail='{}' WHERE id='HN1_1'", [])
            .unwrap();
        let first = store
            .play_page(&PlayQuery {
                page: 0,
                scope: SessionScope::History,
                ..PlayQuery::default()
            })
            .unwrap();
        assert_eq!(first.total_sessions, 25);
        assert_eq!(first.matches.len(), 20);
        assert!(store
            .play_page(&PlayQuery {
                page: 1,
                scope: SessionScope::History,
                ..PlayQuery::default()
            })
            .is_err());
    }
}
