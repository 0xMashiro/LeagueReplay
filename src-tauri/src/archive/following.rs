use super::*;
use crate::domain::following::{FeedEntry, Subscription};
use crate::domain::review::HistoryPage;

impl Store {
    pub fn subscriptions(&self) -> Result<Vec<Subscription>> {
        let mut stmt = self.0.prepare("SELECT s.id,a.id,a.platform,a.puuid,a.summoner_id,a.riot_id,s.server,s.label,s.paused,s.last_synced,s.last_error,s.limited FROM subscriptions s JOIN accounts a ON a.id=s.account_id ORDER BY s.rowid").map_err(sql)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Subscription {
                    id: r.get(0)?,
                    account: Account {
                        id: r.get(1)?,
                        platform: r.get(2)?,
                        puuid: r.get(3)?,
                        summoner_id: r.get(4)?,
                        riot_id: r.get(5)?,
                    },
                    server: r.get(6)?,
                    label: r.get(7)?,
                    paused: r.get(8)?,
                    last_synced: r.get::<_, Option<i64>>(9)?.map(|v| v as f64),
                    last_error: r.get(10)?,
                    limited: r.get(11)?,
                })
            })
            .map_err(sql)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sql)?;
        Ok(rows)
    }

    pub fn subscribe(&mut self, result: &HistoryPage, time: i64) -> Result<String> {
        let a = &result.account;
        if let Some(existing) = self
            .subscriptions()?
            .into_iter()
            .find(|s| s.account.id == a.id)
        {
            return Ok(existing.id);
        }
        if self.subscriptions()?.len() >= 50 {
            return Err("following.limit".into());
        }
        let tx = self.0.transaction().map_err(sql)?;
        tx.execute("INSERT INTO accounts(id,platform,puuid,summoner_id,riot_id) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET riot_id=CASE WHEN excluded.riot_id=excluded.puuid THEN accounts.riot_id ELSE excluded.riot_id END,summoner_id=CASE WHEN excluded.summoner_id='' THEN accounts.summoner_id ELSE excluded.summoner_id END", params![a.id,a.platform,a.puuid,a.summoner_id,a.riot_id]).map_err(sql)?;
        let key = id();
        tx.execute("INSERT INTO subscriptions(id,account_id,server,generation,next_sync) VALUES (?1,?2,?3,?1,?4)", params![key,a.id,result.server,time]).map_err(sql)?;
        Self::cache_follow_page(&tx, &key, result)?;
        let partial = result.skipped_games > 0;
        tx.execute(
            "UPDATE subscriptions SET last_synced=CASE WHEN ?4 THEN NULL ELSE ?2 END,next_sync=?3,last_error=?5,limited=?4 WHERE id=?1",
            params![key, time, time + 300_000, partial, partial.then_some("game.partialHistory")],
        )
        .map_err(sql)?;
        tx.commit().map_err(sql)?;
        Ok(key)
    }

    pub fn edit_subscription(&self, subscription: &str, label: &str, paused: bool) -> Result<()> {
        let label = label.trim();
        if label.chars().count() > 80 || label.chars().any(char::is_control) {
            return Err("following.invalidLabel".into());
        }
        if self.0.execute("UPDATE subscriptions SET label=?2,generation=CASE WHEN paused!=?3 THEN ?4 ELSE generation END,next_sync=CASE WHEN paused=1 AND ?3=0 THEN 0 ELSE next_sync END,paused=?3 WHERE id=?1",params![subscription,label,paused,id()]).map_err(sql)? != 1 { return Err("following.changed".into()); }
        Ok(())
    }

    pub fn unfollow(&self, subscription: &str) -> Result<()> {
        self.0
            .execute("DELETE FROM subscriptions WHERE id=?1", [subscription])
            .map_err(sql)?;
        // Facts, downloaded files, notes and own-play evidence are independent.
        Ok(())
    }

    pub fn follow_ticket(
        &self,
        subscription: &str,
        time: i64,
        force: bool,
    ) -> Result<Option<String>> {
        self.0.query_row("SELECT generation FROM subscriptions WHERE id=?1 AND paused=0 AND (?2 OR next_sync<=?3)",params![subscription,force,time],|r|r.get(0)).optional().map_err(sql)
    }

    pub fn known_follow_game(&self, subscription: &str, game: &str) -> Result<bool> {
        self.0.query_row("SELECT EXISTS(SELECT 1 FROM followed_games WHERE subscription_id=?1 AND game_id=?2)",params![subscription,game],|r|r.get(0)).map_err(sql)
    }

    pub fn finish_follow_sync(
        &mut self,
        subscription: &str,
        ticket: &str,
        pages: &[HistoryPage],
        failure: Option<&str>,
        limited: bool,
        time: i64,
    ) -> Result<bool> {
        let partial = pages.iter().any(|page| page.skipped_games > 0);
        let failure = failure.or(partial.then_some("game.partialHistory"));
        let limited = limited || partial;
        let tx = self.0.transaction().map_err(sql)?;
        let current: Option<(String,String)> = tx.query_row("SELECT account_id,server FROM subscriptions WHERE id=?1 AND generation=?2 AND paused=0",params![subscription,ticket],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(sql)?;
        let Some((account, server)) = current else {
            return Ok(false);
        };
        for page in pages.iter().filter(|_| failure.is_none()) {
            if page.account.id != account || page.server != server {
                return Err("game.identityMismatch".into());
            }
            Self::cache_follow_page(&tx, subscription, page)?;
        }
        tx.execute("UPDATE subscriptions SET last_synced=CASE WHEN ?3 IS NULL THEN ?4 ELSE last_synced END,last_error=?3,limited=MAX(limited,?5),next_sync=?6 WHERE id=?1 AND generation=?2",params![subscription,ticket,failure,time,limited,time+300_000]).map_err(sql)?;
        tx.commit().map_err(sql)?;
        Ok(true)
    }

    pub fn history_cursor(&self, id: &str) -> Result<(u32, Option<String>)> {
        Ok(self
            .0
            .query_row(
                "SELECT next_start,anchor FROM subscription_history WHERE subscription_id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(sql)?
            .unwrap_or((0, None)))
    }
    pub fn finish_history_page(
        &mut self,
        id: &str,
        ticket: &str,
        page: &HistoryPage,
    ) -> Result<bool> {
        let tx = self.0.transaction().map_err(sql)?;
        let valid: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM subscriptions WHERE id=?1 AND generation=?2 AND paused=0 AND account_id=?3 AND server=?4)",params![id,ticket,page.account.id,page.server],|r|r.get(0)).map_err(sql)?;
        if !valid {
            return Ok(false);
        }
        if page.skipped_games > 0 {
            // Do not advance past a gap; a later attempt must revisit this page.
            tx.execute(
                "UPDATE subscriptions SET last_error='game.partialHistory',limited=1 WHERE id=?1",
                [id],
            )
            .map_err(sql)?;
            tx.commit().map_err(sql)?;
            return Err("game.partialHistory".into());
        }
        Self::cache_follow_page(&tx, id, page)?;
        let anchor = page
            .games
            .last()
            .and_then(|g| g["gameId"].as_u64())
            .map(|n| n.to_string());
        tx.execute("INSERT INTO subscription_history(subscription_id,next_start,anchor,done) VALUES(?1,?2,?3,?4) ON CONFLICT(subscription_id) DO UPDATE SET next_start=excluded.next_start,anchor=excluded.anchor,done=excluded.done",params![id,page.start+20,anchor,!page.has_more]).map_err(sql)?;
        tx.execute(
            "UPDATE subscriptions SET limited=?2,last_error=NULL,last_synced=?3 WHERE id=?1",
            params![id, page.has_more, now()],
        )
        .map_err(sql)?;
        tx.commit().map_err(sql)?;
        Ok(true)
    }
    fn cache_follow_page(db: &Connection, subscription: &str, result: &HistoryPage) -> Result<()> {
        let a = &result.account;
        for detail in &result.games {
            let number = detail["gameId"].as_u64().ok_or("game.invalidData")?;
            let game = format!("{}_{number}", a.platform);
            validate_detail(&game, a, detail).map_err(|_| "game.invalidData")?;
            store_participant_accounts(db, &a.platform, detail)?;
            db.execute("INSERT INTO games(id,platform,game_number,detail) VALUES (?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET detail=COALESCE(games.detail,excluded.detail)",params![game,a.platform,number.to_string(),detail.to_string()]).map_err(sql)?;
            db.execute(
                "INSERT OR IGNORE INTO followed_games(subscription_id,game_id) VALUES (?1,?2)",
                params![subscription, game],
            )
            .map_err(sql)?;
        }
        db.execute(
            "UPDATE accounts SET riot_id=CASE WHEN ?2=puuid THEN riot_id ELSE ?2 END,summoner_id=CASE WHEN ?3='' THEN summoner_id ELSE ?3 END WHERE id=?1",
            params![a.id, a.riot_id, a.summoner_id],
        )
        .map_err(sql)?;
        Ok(())
    }

    pub fn follow_feed(
        &self,
        subscription: Option<&str>,
        unread: bool,
        start: u32,
    ) -> Result<Vec<FeedEntry>> {
        if start > 10000 {
            return Err("query.invalidPage".into());
        }
        let mut stmt=self.0.prepare("SELECT f.subscription_id,f.game_id,a.puuid FROM followed_games f JOIN subscriptions s ON s.id=f.subscription_id JOIN accounts a ON a.id=s.account_id JOIN games g ON g.id=f.game_id WHERE (?1 IS NULL OR s.id=?1) AND (?2=0 OR NOT EXISTS(SELECT 1 FROM user_state u,json_each(u.data,'$.reviewed') j WHERE j.value=f.game_id)) ORDER BY json_extract(g.detail,'$.gameCreation') DESC,f.game_id DESC,s.id LIMIT 50 OFFSET ?3").map_err(sql)?;
        let rows = stmt
            .query_map(params![subscription, unread, start], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(sql)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sql)?;
        rows.into_iter()
            .map(|(subscription_id, id, puuid)| {
                let game = self
                    .cached_review(&id, Some(&puuid))?
                    .ok_or("library.notFound")?;
                Ok(FeedEntry {
                    subscription_id,
                    game: super::review::library_entry(game)?,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::tests::{account, detail, workspace};
    fn page(numbers: &[u32]) -> HistoryPage {
        HistoryPage {
            account: account("a"),
            server: "TENCENT_HN1".into(),
            games: numbers.iter().map(|n| {
                let mut game = detail("a", *n);
                game["participantIdentities"][0]["player"] = serde_json::json!({"puuid":"a","summonerId":1,"gameName":"a","tagLine":"TEST"});
                game
            }).collect(),
            start: 0,
            has_more: false,
            skipped_games: 0,
            source: "lcu".into(),
        }
    }
    #[test]
    fn following_is_persistent_study_data_and_does_not_claim_play() {
        let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
        let id = store.subscribe(&page(&[1, 2]), 1000).unwrap();
        assert_eq!(store.subscribe(&page(&[1, 2]), 1001).unwrap(), id);
        assert_eq!(store.subscriptions().unwrap().len(), 1);
        assert_eq!(store.follow_feed(None, false, 0).unwrap().len(), 2);
        assert!(workspace(&store).matches.is_empty());
        assert!(store.library(0, "recent", "").unwrap().is_empty());
        let game = store.cached_review("HN1_1", Some("a")).unwrap().unwrap();
        store.save_review(&game, 1002).unwrap();
        store.bookmark("HN1_1", true).unwrap();
        let mut ui = LiveUiState::default();
        ui.reviewed.push("HN1_1".into());
        store.save_ui(&ui, 0).unwrap();
        assert_eq!(store.follow_feed(None, true, 0).unwrap().len(), 1);
        store.unfollow(&id).unwrap();
        assert!(store.follow_feed(None, false, 0).unwrap().is_empty());
        assert_eq!(store.library(0, "saved", "").unwrap().len(), 1);
        assert_eq!(workspace(&store).ui.reviewed.len(), 1);
    }
    #[test]
    fn empty_cross_region_sync_preserves_known_account_details() {
        let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
        let original = page(&[1]);
        let id = store.subscribe(&original, 1000).unwrap();
        let ticket = store.follow_ticket(&id, 1001, true).unwrap().unwrap();
        let mut empty = page(&[]);
        empty.source = "sgp".into();
        empty.account.riot_id = empty.account.puuid.clone();
        empty.account.summoner_id.clear();
        store
            .finish_follow_sync(&id, &ticket, &[empty.clone()], None, false, 1002)
            .unwrap();
        let saved = store.subscriptions().unwrap();
        assert_eq!(saved[0].account.riot_id, original.account.riot_id);
        assert_eq!(saved[0].account.summoner_id, original.account.summoner_id);
        assert_eq!(store.follow_feed(None, false, 0).unwrap().len(), 1);
        store.unfollow(&id).unwrap();
        store.subscribe(&empty, 1003).unwrap();
        let saved = store.subscriptions().unwrap();
        assert_eq!(saved[0].account.riot_id, original.account.riot_id);
        assert_eq!(saved[0].account.summoner_id, original.account.summoner_id);
    }
    #[test]
    fn interrupted_sync_preserves_cursor_and_stale_results_cannot_revive_a_subscription() {
        let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
        let id = store.subscribe(&page(&[1]), 1000).unwrap();
        assert!(store.follow_ticket(&id, 1001, false).unwrap().is_none());
        let ticket = store.follow_ticket(&id, 1001, true).unwrap().unwrap();
        store
            .finish_follow_sync(
                &id,
                &ticket,
                &[page(&[2])],
                Some("gateway.network"),
                false,
                2000,
            )
            .unwrap();
        assert_eq!(store.follow_feed(None, false, 0).unwrap().len(), 1);
        assert_eq!(store.subscriptions().unwrap()[0].last_synced, Some(1000.0));
        store.edit_subscription(&id, "练习", true).unwrap();
        assert!(!store
            .finish_follow_sync(&id, &ticket, &[page(&[2])], None, false, 3000)
            .unwrap());
        store.edit_subscription(&id, "练习", false).unwrap();
        let fresh = store.follow_ticket(&id, 3001, false).unwrap().unwrap();
        assert_ne!(fresh, ticket);
        store
            .finish_follow_sync(&id, &fresh, &[page(&[2, 3])], None, false, 4000)
            .unwrap();
        assert_eq!(store.follow_feed(None, false, 0).unwrap().len(), 3);
        store.unfollow(&id).unwrap();
        let new = store.subscribe(&page(&[1]), 5000).unwrap();
        assert_ne!(new, id);
        assert!(!store
            .finish_follow_sync(&id, &fresh, &[page(&[2])], None, false, 5001)
            .unwrap());
    }
    #[test]
    fn history_page_commits_cursor_with_facts_and_rejects_stale_responses() {
        let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
        let id = store.subscribe(&page(&[1]), 1000).unwrap();
        let ticket = store.follow_ticket(&id, 1001, true).unwrap().unwrap();
        let mut older = page(&[2, 3]);
        older.start = 20;
        older.has_more = true;
        assert!(store.finish_history_page(&id, &ticket, &older).unwrap());
        assert_eq!(store.history_cursor(&id).unwrap(), (40, Some("3".into())));
        let mut invalid = page(&[4, 5]);
        invalid.games[1]["gameId"] = serde_json::json!(0);
        assert!(store.finish_history_page(&id, &ticket, &invalid).is_err());
        assert!(!store.known_follow_game(&id, "HN1_4").unwrap());
        assert_eq!(store.history_cursor(&id).unwrap().0, 40);
        store.edit_subscription(&id, "Study", true).unwrap();
        assert!(!store
            .finish_history_page(&id, &ticket, &page(&[6]))
            .unwrap());
        assert!(!store.known_follow_game(&id, "HN1_6").unwrap());
    }
    #[test]
    fn partial_pages_do_not_advance_sync_or_history_and_can_be_retried() {
        let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
        let id = store.subscribe(&page(&[1]), 1000).unwrap();
        let ticket = store.follow_ticket(&id, 1001, true).unwrap().unwrap();
        let mut partial = page(&[2]);
        partial.skipped_games = 1;
        store
            .finish_follow_sync(&id, &ticket, &[partial.clone()], None, false, 2000)
            .unwrap();
        let subscription = store.subscriptions().unwrap().remove(0);
        assert_eq!(subscription.last_synced, Some(1000.0));
        assert_eq!(
            subscription.last_error.as_deref(),
            Some("game.partialHistory")
        );
        assert!(subscription.limited);
        assert!(!store.known_follow_game(&id, "HN1_2").unwrap());
        assert_eq!(
            store
                .finish_history_page(&id, &ticket, &partial)
                .unwrap_err(),
            "game.partialHistory"
        );
        assert_eq!(store.history_cursor(&id).unwrap(), (0, None));
        assert!(store
            .finish_history_page(&id, &ticket, &page(&[2, 3]))
            .unwrap());
        assert_eq!(store.history_cursor(&id).unwrap(), (20, Some("3".into())));
        let subscription = store.subscriptions().unwrap().remove(0);
        assert!(!subscription.limited && subscription.last_error.is_none());
        assert!(store.known_follow_game(&id, "HN1_2").unwrap());
    }
    #[test]
    fn following_from_partial_search_keeps_valid_games_but_reports_the_gap() {
        let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
        let mut partial = page(&[1]);
        partial.skipped_games = 1;
        let id = store.subscribe(&partial, 1000).unwrap();
        let subscription = store.subscriptions().unwrap().remove(0);
        assert!(subscription.last_synced.is_none() && subscription.limited);
        assert_eq!(
            subscription.last_error.as_deref(),
            Some("game.partialHistory")
        );
        assert!(store.known_follow_game(&id, "HN1_1").unwrap());
        assert_eq!(store.history_cursor(&id).unwrap(), (0, None));
    }
    #[test]
    fn invalid_pages_rollback_and_sync_keeps_richer_review_data() {
        let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
        let mut invalid = page(&[1, 2]);
        invalid.games[1]["platformId"] = serde_json::json!("HN10");
        assert!(store.subscribe(&invalid, 1000).is_err());
        assert!(store.subscriptions().unwrap().is_empty());
        let id = store.subscribe(&page(&[1]), 1000).unwrap();
        let mut game = store.cached_review("HN1_1", Some("a")).unwrap().unwrap();
        game.timeline = Some(serde_json::json!({"frames":[]}));
        game.detail["extra"] = serde_json::json!("richer");
        store.save_review(&game, 2000).unwrap();
        let ticket = store.follow_ticket(&id, 2000, true).unwrap().unwrap();
        store
            .finish_follow_sync(&id, &ticket, &[page(&[1, 2])], None, false, 3000)
            .unwrap();
        let saved = store.cached_review("HN1_1", Some("a")).unwrap().unwrap();
        assert!(saved.timeline.is_some());
        assert_eq!(saved.detail["extra"], "richer");
    }
    #[test]
    fn library_search_filters_before_paging_and_backup_preserves_study_state() {
        let mut store = Store::new(Connection::open_in_memory().unwrap()).unwrap();
        let numbers = (1..=60).collect::<Vec<_>>();
        let id = store.subscribe(&page(&numbers), 1000).unwrap();
        store.edit_subscription(&id, "Study", false).unwrap();
        let ticket = store.follow_ticket(&id, 1001, true).unwrap().unwrap();
        for number in numbers {
            let game = store
                .cached_review(&format!("HN1_{number}"), Some("a"))
                .unwrap()
                .unwrap();
            store.save_review(&game, i64::from(number)).unwrap();
        }
        let mut ui = LiveUiState::default();
        ui.notes.push(LiveNote {
            id: "note".into(),
            match_id: "HN1_1".into(),
            body: "rare decision".into(),
            at: Some(42.0),
            participant_id: Some(1),
            tags: vec!["波次".into()],
            updated_at: "2026-09-21T00:00:00Z".into(),
        });
        ui.reviewed.push("HN1_1".into());
        ui.players.push(PlayerProfile {
            id: "person".into(),
            name: "Friend".into(),
            account_ids: vec!["HN1:a".into()],
        });
        store.save_ui(&ui, 0).unwrap();
        store.bookmark("HN1_1", true).unwrap();
        assert_eq!(store.library(0, "recent", "").unwrap().len(), 50);
        assert_eq!(store.library(50, "recent", "").unwrap().len(), 10);
        assert_eq!(
            store.library(0, "recent", "rare decision").unwrap()[0].id,
            "HN1_1"
        );
        assert_eq!(store.library(0, "saved", "波次").unwrap().len(), 1);
        assert!(store.library(0, "recent", "not found").unwrap().is_empty());
        let mut raw = Vec::new();
        store.write_backup(&mut raw).unwrap();
        let (source, date) = Store::read_backup(raw.as_slice()).unwrap();
        let preview = store.backup_preview(&source, date).unwrap();
        let directory = std::env::temp_dir().join(format!(
            "league-replay-follow-backup-{}",
            uuid::Uuid::new_v4()
        ));
        let copy = store
            .restore_snapshot(&source, &preview.fingerprint, &directory)
            .unwrap();
        assert_eq!(store.subscriptions().unwrap()[0].label, "Study");
        assert_eq!(store.library(0, "saved", "").unwrap().len(), 1);
        assert_eq!(store.follow_feed(None, true, 50).unwrap().len(), 9);
        assert!(!store
            .finish_follow_sync(&id, &ticket, &[page(&[99])], None, false, 5000)
            .unwrap());
        let restored = workspace(&store);
        assert!(restored.matches.is_empty());
        assert_eq!(restored.ui.notes[0].body, "rare decision");
        assert_eq!(restored.ui.players[0].account_ids, vec!["HN1:a"]);
        std::fs::remove_file(copy).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
