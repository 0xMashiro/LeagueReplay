use super::*;

pub(super) fn install(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE workspace_clock(id INTEGER PRIMARY KEY CHECK(id=1),value INTEGER NOT NULL); INSERT INTO workspace_clock VALUES(1,0); CREATE TABLE workspace_changes(kind TEXT NOT NULL,id TEXT NOT NULL,revision INTEGER NOT NULL,deleted INTEGER NOT NULL,PRIMARY KEY(kind,id)); CREATE INDEX workspace_revision ON workspace_changes(revision); CREATE INDEX participation_segment ON participations(segment_id);") .map_err(sql)?;
    for (table, kind) in [("participations", "participation"), ("games", "game")] {
        for (event, row, deleted) in [
            ("INSERT", "NEW", 0),
            ("UPDATE", "NEW", 0),
            ("DELETE", "OLD", 1),
        ] {
            db.execute_batch(&format!("CREATE TRIGGER sync_{table}_{event} AFTER {event} ON {table} BEGIN UPDATE workspace_clock SET value=value+1 WHERE id=1; INSERT INTO workspace_changes(kind,id,revision,deleted) VALUES('{kind}',{row}.id,(SELECT value FROM workspace_clock WHERE id=1),{deleted}) ON CONFLICT(kind,id) DO UPDATE SET revision=excluded.revision,deleted=excluded.deleted; END;" )).map_err(sql)?;
        }
    }
    for (table, columns) in [
        (
            "accounts",
            &["id", "platform", "puuid", "summoner_id", "riot_id"][..],
        ),
        (
            "sessions",
            &["id", "title", "started_at", "ended_at", "end_reason"][..],
        ),
        ("segments", &["id", "session_id", "account_id"][..]),
        ("user_state", &["data", "revision"][..]),
    ] {
        for event in ["INSERT", "UPDATE", "DELETE"] {
            let condition = if event == "UPDATE" {
                format!(
                    "WHEN {}",
                    columns
                        .iter()
                        .map(|column| format!("OLD.{column} IS NOT NEW.{column}"))
                        .collect::<Vec<_>>()
                        .join(" OR ")
                )
            } else {
                String::new()
            };
            db.execute_batch(&format!("CREATE TRIGGER sync_{table}_{event} AFTER {event} ON {table} {condition} BEGIN UPDATE workspace_clock SET value=value+1 WHERE id=1; END;")).map_err(sql)?;
        }
    }
    Ok(())
}

impl Store {
    pub fn sync_workspace(
        &self,
        status: ClientStatus,
        cursor: Option<f64>,
    ) -> Result<WorkspaceUpdate> {
        let current: i64 = self
            .0
            .query_row("SELECT value FROM workspace_clock WHERE id=1", [], |r| {
                r.get(0)
            })
            .map_err(sql)?;
        let since = cursor
            .filter(|v| v.is_finite() && *v >= 0.0 && v.fract() == 0.0 && *v <= current as f64)
            .map(|v| v as i64);
        if since == Some(current) {
            return Ok(WorkspaceUpdate {
                cursor: current as f64,
                reset: false,
                removed: Vec::new(),
                status,
                workspace: None,
            });
        }
        let workspace = self.workspace_since(status.clone(), since)?;
        let mut stmt = self.0.prepare("SELECT id FROM workspace_changes WHERE kind='participation' AND deleted=1 AND revision>?1").map_err(sql)?;
        let removed = stmt
            .query_map([since.unwrap_or(current)], |r| r.get::<_, String>(0))
            .map_err(sql)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sql)?;
        Ok(WorkspaceUpdate {
            cursor: current as f64,
            reset: since.is_none(),
            removed,
            status,
            workspace: Some(workspace),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::tests::{detail, sample};
    fn status() -> ClientStatus {
        ClientStatus::unavailable("offline", "test", 0)
    }
    #[test]
    fn delta_carries_only_changed_matches_and_preserves_full_session_membership() {
        let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
        store.observe(Some(&sample("a", 1)), 1000).unwrap();
        store
            .complete(
                "HN1_1",
                &crate::archive::tests::account("a"),
                &detail("a", 1),
                Err("client.NotFound"),
                1000,
            )
            .unwrap();
        let initial = store.sync_workspace(status(), None).unwrap();
        assert!(initial.reset);
        assert_eq!(initial.workspace.as_ref().unwrap().matches.len(), 1);
        let unchanged = store
            .sync_workspace(status(), Some(initial.cursor))
            .unwrap();
        assert!(unchanged.workspace.is_none());
        assert_eq!(
            initial.workspace.as_ref().unwrap().sessions[0].segments[0]
                .match_ids
                .len(),
            1
        );
        store.observe(Some(&sample("b", 2)), 2000).unwrap();
        let update = store
            .sync_workspace(status(), Some(initial.cursor))
            .unwrap();
        assert_eq!(update.workspace.as_ref().unwrap().matches.len(), 1);
        assert_eq!(update.workspace.as_ref().unwrap().matches[0].id, "HN1_2");
        let id = update.workspace.as_ref().unwrap().matches[0]
            .observation_id
            .clone();
        store
            .0
            .execute("DELETE FROM participations WHERE id=?1", [&id])
            .unwrap();
        let deleted = store.sync_workspace(status(), Some(update.cursor)).unwrap();
        assert_eq!(deleted.removed, vec![id]);
        assert!(deleted.workspace.as_ref().unwrap().matches.is_empty());
        assert!(
            store
                .sync_workspace(status(), Some(f64::INFINITY))
                .unwrap()
                .reset
        );
    }

    #[test]
    fn workspace_omits_timeline_but_review_reads_the_saved_events() {
        let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
        let account = crate::archive::tests::account("a");
        let mut game = detail("a", 1);
        game["participantIdentities"][0]["player"] =
            serde_json::json!({"puuid":"a","summonerId":1,"gameName":"a","tagLine":"TEST"});
        let timeline = crate::matches::lcu_timeline("TENCENT_HN1", "1", serde_json::json!({"frames":[{"events":[{"type":"ITEM_PURCHASED","timestamp":42000,"itemId":1001,"participantId":1}]}]})).unwrap();
        store.observe(Some(&sample("a", 1)), 1000).unwrap();
        store
            .complete("HN1_1", &account, &game, Err("client.Unavailable"), 2000)
            .unwrap();
        assert!(store.pending("HN1", 61_999).unwrap().is_none());
        assert_eq!(
            store
                .cached_review("HN1_1", Some("a"))
                .unwrap()
                .unwrap()
                .timeline_error
                .as_deref(),
            Some("client.Unavailable")
        );
        assert_eq!(store.pending("HN1", 62_000).unwrap().unwrap().0, "HN1_1");
        store
            .complete("HN1_1", &account, &game, Ok(&timeline), 62_000)
            .unwrap();
        assert!(store.pending("HN1", 62_000).unwrap().is_none());
        let update = store.sync_workspace(status(), None).unwrap();
        let wire = serde_json::to_value(&update).unwrap();
        assert!(wire["workspace"]["matches"][0].get("timeline").is_none());
        assert!(wire["workspace"]["matches"][0].get("detail").is_none());
        assert_eq!(
            wire["workspace"]["matches"][0]["game"]["participants"][0]["puuid"],
            "a"
        );
        let saved = store.cached_review("HN1_1", Some("a")).unwrap().unwrap();
        assert_eq!(saved.timeline, Some(timeline));
        let review = crate::matches::review_game(saved).unwrap();
        let events = review.timeline.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].at, 42);
    }

    #[test]
    fn metadata_changes_advance_cursor_but_noop_writes_and_status_do_not() {
        let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
        store.observe(Some(&sample("a", 1)), 1000).unwrap();
        let initial = store.sync_workspace(status(), None).unwrap();
        let mut cursor = initial.cursor;
        store
            .0
            .execute("UPDATE accounts SET riot_id=riot_id", [])
            .unwrap();
        store
            .0
            .execute("UPDATE sessions SET last_activity=2000", [])
            .unwrap();
        let status_only = store
            .sync_workspace(
                ClientStatus::unavailable("connected", "fresh", 4000),
                Some(cursor),
            )
            .unwrap();
        assert!(status_only.workspace.is_none());
        assert_eq!(status_only.status.last_checked_at, 4000.0);
        assert_eq!(status_only.cursor, cursor);
        for statement in [
            "UPDATE accounts SET riot_id='Renamed#TEST'",
            "UPDATE sessions SET title='Renamed session'",
            "INSERT INTO sessions VALUES('other','Other',0,0,0,'import')",
            "UPDATE segments SET session_id='other'",
            "UPDATE user_state SET revision=revision+1",
            "DELETE FROM sessions WHERE id<>'other'",
        ] {
            store.0.execute(statement, []).unwrap();
            let changed = store.sync_workspace(status(), Some(cursor)).unwrap();
            assert!(changed.cursor > cursor, "{statement}");
            assert!(
                changed.workspace.unwrap().matches.is_empty(),
                "metadata must not resend game details"
            );
            cursor = changed.cursor;
        }
        let tx = store.0.transaction().unwrap();
        tx.execute("UPDATE sessions SET title='Rolled back'", [])
            .unwrap();
        tx.rollback().unwrap();
        assert!(store
            .sync_workspace(status(), Some(cursor))
            .unwrap()
            .workspace
            .is_none());
    }
}
