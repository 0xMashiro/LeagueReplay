use crate::{
    archive::{store_participant_accounts, validate_detail, Store},
    domain::Account,
};
use crate::{
    domain::review::{LibraryEntry, StoredReview},
    matches as data,
};
use rusqlite::{params, OptionalExtension};
use serde_json::Value;

fn error(_: rusqlite::Error) -> String {
    "library.storageError".into()
}

fn record_view(
    db: &rusqlite::Transaction<'_>,
    game: &StoredReview,
    now: i64,
) -> Result<(), String> {
    let a = &game.account;
    db.execute("INSERT INTO accounts(id,platform,puuid,summoner_id,riot_id) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET riot_id=excluded.riot_id,summoner_id=excluded.summoner_id",params![a.id,a.platform,a.puuid,a.summoner_id,a.riot_id]).map_err(error)?;
    db.execute("INSERT INTO review_games(game_id,account_id,server,source,timeline_error,viewed_at) VALUES (?1,?2,?3,?4,?5,?6) ON CONFLICT(game_id) DO UPDATE SET account_id=excluded.account_id,server=excluded.server,source=excluded.source,viewed_at=excluded.viewed_at",params![game.id,a.id,game.server,game.source,game.timeline_error,now]).map_err(error)?;
    Ok(())
}

impl Store {
    pub fn review_notes(&self, id: &str) -> Result<Vec<crate::domain::LiveNote>, String> {
        let raw: String = self
            .0
            .query_row("SELECT data FROM user_state WHERE id=1", [], |row| {
                row.get(0)
            })
            .map_err(error)?;
        let ui: crate::domain::LiveUiState =
            serde_json::from_str(&raw).map_err(|_| "library.storageError")?;
        Ok(ui
            .notes
            .into_iter()
            .filter(|note| note.match_id == id)
            .collect())
    }
}

pub(super) fn library_entry(game: StoredReview) -> Result<LibraryEntry, String> {
    let participant = validate_detail(&game.id, &game.account, &game.detail)
        .map_err(|_| "library.invalidData")?;
    let n = |v: &Value| v.as_u64().unwrap_or(0) as u32;
    let stats = &participant["stats"];
    Ok(LibraryEntry {
        id: game.id,
        server: game.server,
        champion_id: n(&participant["championId"]),
        started_at: game.detail["gameCreation"].as_f64().unwrap_or(0.0),
        duration: n(&game.detail["gameDuration"]),
        win: stats["win"].as_bool().unwrap(),
        kills: n(&stats["kills"]),
        deaths: n(&stats["deaths"]),
        assists: n(&stats["assists"]),
        items: (0..7).map(|i| n(&stats[format!("item{i}")])).collect(),
        queue_id: n(&game.detail["queueId"]),
        version: game.detail["gameVersion"].as_str().unwrap_or("").into(),
        bookmarked: game.bookmarked,
        account: game.account,
    })
}

impl Store {
    pub fn cached_review(
        &self,
        id: &str,
        puuid: Option<&str>,
    ) -> Result<Option<StoredReview>, String> {
        if let Some(game) = self.review(id, puuid)? {
            return Ok(Some(game));
        }
        let row = self
            .0
            .query_row(
                "SELECT platform,detail,timeline,data_error FROM games WHERE id=?1 AND detail IS NOT NULL",
                [id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<String>>(2)?,
                        r.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(error)?;
        row.map(|(platform,detail,timeline,data_error)| {
            let detail:Value=serde_json::from_str(&detail).map_err(|_| "library.invalidData")?;
            let timeline=timeline.map(|v|serde_json::from_str::<Value>(&v)).transpose().map_err(|_| "library.invalidData")?;
            let observed:Option<String>=self.0.query_row("SELECT a.puuid FROM participations p JOIN accounts a ON a.id=p.account_id WHERE p.game_id=?1 ORDER BY first_seen LIMIT 1",[id],|r|r.get(0)).optional().map_err(error)?;
            let perspective=puuid.or(observed.as_deref()).or_else(||detail["participantIdentities"][0]["player"]["puuid"].as_str()).ok_or("game.playerMissing")?;
            let account=data::account_in_game(&platform,perspective,&detail)?;
            let timeline_error = if timeline.is_none() { data_error } else { None };
            Ok(StoredReview{id:id.into(),server:crate::regions::server_for_platform(&platform)?,account,detail,timeline,source:"archive".into(),timeline_error,bookmarked:false})
        }).transpose()
    }

    pub fn visit_review(
        &mut self,
        id: &str,
        puuid: Option<&str>,
        now: i64,
    ) -> Result<Option<StoredReview>, String> {
        let Some(game) = self.cached_review(id, puuid)? else {
            return Ok(None);
        };
        let tx = self.0.transaction().map_err(error)?;
        record_view(&tx, &game, now)?;
        tx.commit().map_err(error)?;
        Ok(Some(game))
    }

    pub fn save_review(&mut self, game: &StoredReview, now: i64) -> Result<(), String> {
        validate_detail(&game.id, &game.account, &game.detail).map_err(|_| "game.invalidData")?;
        let tx = self.0.transaction().map_err(error)?;
        let a = &game.account;
        store_participant_accounts(&tx, &a.platform, &game.detail)?;
        tx.execute("INSERT INTO games(id,platform,game_number,detail) VALUES (?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET detail=excluded.detail",params![game.id,a.platform,game.detail["gameId"].as_u64().unwrap().to_string(),game.detail.to_string()]).map_err(error)?;
        record_view(&tx, game, now)?;
        super::save_timeline(
            &tx,
            &game.id,
            game.timeline.as_ref(),
            game.timeline_error.as_deref(),
            now,
        )?;
        tx.commit().map_err(error)
    }

    pub fn review(&self, id: &str, puuid: Option<&str>) -> Result<Option<StoredReview>, String> {
        let row=self.0.query_row("SELECT r.server,r.source,r.timeline_error,r.bookmarked,a.id,a.platform,a.puuid,a.summoner_id,a.riot_id,g.detail,g.timeline FROM review_games r JOIN games g ON g.id=r.game_id JOIN accounts a ON a.id=r.account_id WHERE g.id=?1",[id],|r| {
            Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?,r.get::<_,bool>(3)?,Account{id:r.get(4)?,platform:r.get(5)?,puuid:r.get(6)?,summoner_id:r.get(7)?,riot_id:r.get(8)?},r.get::<_,String>(9)?,r.get::<_,Option<String>>(10)?))
        }).optional().map_err(error)?;
        row.map(
            |(server, source, timeline_error, bookmarked, account, detail, timeline)| {
                let detail: Value =
                    serde_json::from_str(&detail).map_err(|_| "library.invalidData")?;
                let timeline: Option<Value> = timeline
                    .map(|v| serde_json::from_str(&v))
                    .transpose()
                    .map_err(|_| "library.invalidData")?;
                let account = if let Some(puuid) = puuid {
                    data::account_in_game(&account.platform, puuid, &detail)?
                } else {
                    account
                };
                Ok(StoredReview {
                    id: id.into(),
                    server,
                    source,
                    timeline_error: if timeline.is_some() {
                        None
                    } else {
                        timeline_error
                    },
                    bookmarked,
                    account,
                    detail,
                    timeline,
                })
            },
        )
        .transpose()
    }

    pub fn library(
        &self,
        start: u32,
        filter: &str,
        query: &str,
    ) -> Result<Vec<LibraryEntry>, String> {
        if start > 10000 || !["recent", "saved", "downloaded"].contains(&filter) {
            return Err("query.invalidPage".into());
        }
        if query.chars().count() > 200 {
            return Err("query.invalidPage".into());
        }
        let mut stmt=self.0.prepare("SELECT r.game_id FROM review_games r JOIN accounts a ON a.id=r.account_id WHERE (?1!='saved' OR r.bookmarked=1) AND (?1!='downloaded' OR EXISTS(SELECT 1 FROM replay_files f WHERE f.game_id=r.game_id)) AND (?3='' OR instr(lower(a.riot_id||' '||a.platform||' '||r.game_id),lower(?3))>0 OR EXISTS(SELECT 1 FROM user_state u,json_each(u.data,'$.notes') n WHERE json_extract(n.value,'$.matchId')=r.game_id AND instr(lower(json_extract(n.value,'$.body')||' '||json_extract(n.value,'$.tags')),lower(?3))>0)) ORDER BY r.viewed_at DESC,r.game_id DESC LIMIT 50 OFFSET ?2").map_err(error)?;
        let ids = stmt
            .query_map(params![filter, start, query.trim()], |r| {
                r.get::<_, String>(0)
            })
            .map_err(error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(error)?;
        ids.into_iter()
            .map(|id| {
                let game = self.review(&id, None)?.ok_or("library.notFound")?;
                library_entry(game)
            })
            .collect()
    }

    pub fn bookmark(&self, id: &str, value: bool) -> Result<(), String> {
        let count = self
            .0
            .execute(
                "UPDATE review_games SET bookmarked=?2 WHERE game_id=?1",
                params![id, value],
            )
            .map_err(error)?;
        if count == 0 {
            return Err("library.notFound".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use serde_json::json;
    #[test]
    fn missing_timeline_retries_without_claiming_play_or_losing_saved_data() {
        let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
        let account = crate::archive::tests::account("a");
        let mut detail = crate::archive::tests::detail("a", 42);
        detail["participantIdentities"][0]["player"]["summonerId"] = json!(1);
        let game = StoredReview {
            id: "HN1_42".into(),
            server: "TENCENT_HN1".into(),
            source: "lcu".into(),
            account: account.clone(),
            detail: detail.clone(),
            timeline: None,
            timeline_error: Some("game.invalidTimeline".into()),
            bookmarked: false,
        };
        store.save_review(&game, 1000).unwrap();
        {
            // Reading a cached game is not another fetch attempt and must not
            // postpone its already scheduled background retry.
            store.visit_review(&game.id, None, 60_000).unwrap();
        }
        assert_eq!(
            store
                .cached_review(&game.id, None)
                .unwrap()
                .unwrap()
                .timeline_error,
            game.timeline_error
        );
        store.bookmark(&game.id, true).unwrap();
        assert!(store.pending("HN1", 60_999).unwrap().is_none());
        assert!(store.pending("HN10", 61_000).unwrap().is_none());
        assert_eq!(store.pending("HN1", 61_000).unwrap().unwrap().0, game.id);
        store
            .failed(&game.id, 61_000, "client.Unavailable")
            .unwrap();
        assert!(store.pending("HN1", 120_999).unwrap().is_none());
        assert!(store.pending("HN1", 121_000).unwrap().is_some());
        assert_eq!(
            store
                .review(&game.id, None)
                .unwrap()
                .unwrap()
                .timeline_error
                .as_deref(),
            Some("client.Unavailable")
        );
        let timeline =
            crate::matches::lcu_timeline("TENCENT_HN1", "42", json!({"frames":[]})).unwrap();
        store
            .complete(&game.id, &account, &detail, Ok(&timeline), 121_000)
            .unwrap();
        store
            .complete(
                &game.id,
                &account,
                &detail,
                Err("client.Unavailable"),
                122_000,
            )
            .unwrap();
        // A late unsuccessful page fetch must also preserve the successful timeline.
        store.save_review(&game, 123_000).unwrap();
        store
            .failed(&game.id, 124_000, "client.Unavailable")
            .unwrap();
        let error: Option<String> = store
            .0
            .query_row(
                "SELECT data_error FROM games WHERE id=?1",
                [&game.id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(error.is_none());
        assert!(store.pending("HN1", 999_999).unwrap().is_none());
        let saved = store.review(&game.id, None).unwrap().unwrap();
        assert!(saved.bookmarked && saved.timeline_error.is_none());
        assert_eq!(saved.timeline, Some(timeline));
        let count: u32 = store
            .0
            .query_row("SELECT COUNT(*) FROM participations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn both_timeline_writers_roll_back_detail_when_timeline_storage_fails() {
        for background in [false, true] {
            let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
            let account = crate::archive::tests::account("a");
            let mut detail = crate::archive::tests::detail("a", 42);
            detail["participantIdentities"][0]["player"]["summonerId"] = json!(1);
            let original = StoredReview {
                id: "HN1_42".into(),
                server: "TENCENT_HN1".into(),
                source: "lcu".into(),
                account,
                detail,
                timeline: None,
                timeline_error: Some("client.NotFound".into()),
                bookmarked: false,
            };
            store.save_review(&original, 1000).unwrap();
            store.0.execute_batch("CREATE TEMP TRIGGER fail_timeline BEFORE UPDATE OF timeline ON games BEGIN SELECT RAISE(ABORT,'timeline write failed'); END;").unwrap();
            let mut fresh = original.clone();
            fresh.detail["gameDuration"] = json!(1800);
            fresh.timeline = Some(json!({"frames":[]}));
            fresh.timeline_error = None;
            let result = if background {
                store.complete(
                    &fresh.id,
                    &fresh.account,
                    &fresh.detail,
                    Ok(fresh.timeline.as_ref().unwrap()),
                    2000,
                )
            } else {
                store.save_review(&fresh, 2000)
            };
            assert!(result.is_err());
            let saved = store.review(&original.id, None).unwrap().unwrap();
            assert_eq!(saved.detail, original.detail);
            assert_eq!(saved.timeline, original.timeline);
            assert_eq!(saved.timeline_error, original.timeline_error);
            assert!(store.pending("HN1", 60_999).unwrap().is_none());
            assert!(store.pending("HN1", 61_000).unwrap().is_some());
        }
    }
    #[test]
    fn studying_does_not_claim_play_and_later_backfill_keeps_saved_facts() {
        let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
        let account = Account {
            id: "HN1:other".into(),
            platform: "HN1".into(),
            puuid: "other".into(),
            summoner_id: "1".into(),
            riot_id: "Player#TAG".into(),
        };
        let detail = json!({"platformId":"HN1","gameId":42,"gameCreation":1000,"gameDuration":1800,"participantIdentities":[{"participantId":1,"player":{"puuid":"other","summonerId":1,"gameName":"Player","tagLine":"TAG"}}],"participants":[{"participantId":1,"championId":103,"teamId":100,"stats":{"win":true}}]});
        let mut game = StoredReview {
            id: "HN1_42".into(),
            server: "TENCENT_HN1".into(),
            source: "lcu".into(),
            account,
            detail,
            timeline: Some(json!({"gameId":42,"frames":[]})),
            timeline_error: None,
            bookmarked: false,
        };
        store.save_review(&game, 100).unwrap();
        store.bookmark(&game.id, true).unwrap();
        let count: i64 = store
            .0
            .query_row("SELECT COUNT(*) FROM participations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
        game.timeline = None;
        game.timeline_error = Some("gateway.network".into());
        store.save_review(&game, 200).unwrap();
        let restored = store.review(&game.id, None).unwrap().unwrap();
        assert!(
            restored.bookmarked && restored.timeline.is_some() && restored.timeline_error.is_none()
        );
        store
            .import(&game.account, &game.detail, None, None, 300)
            .unwrap();
        let observation: String = store
            .0
            .query_row("SELECT id FROM participations", [], |r| r.get(0))
            .unwrap();
        store.retract_manual(&observation).unwrap();
        assert_eq!(store.library(0, "saved", "").unwrap().len(), 1);
        assert!(store
            .review(&game.id, None)
            .unwrap()
            .unwrap()
            .timeline
            .is_some());
    }
}
