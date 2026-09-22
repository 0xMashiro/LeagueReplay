use super::*;

pub(super) fn validate_store(store: &mut Store) -> Result<()> {
    let ui: (String, u32) = store
        .0
        .query_row("SELECT data,revision FROM user_state WHERE id=1", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .map_err(invalid)?;
    let original_ui = ui.0;
    let ui: LiveUiState = serde_json::from_str(&original_ui).map_err(invalid)?;
    // Reuse normal write validation in this disposable database only.
    let revision = store
        .0
        .query_row("SELECT revision FROM user_state WHERE id=1", [], |r| {
            r.get(0)
        })
        .map_err(invalid)?;
    if revision == u32::MAX {
        return Err("backup.invalid".into());
    }
    store.save_ui(&ui, revision).map_err(invalid)?;
    store
        .0
        .execute(
            "UPDATE user_state SET data=?1,revision=?2 WHERE id=1",
            params![original_ui, revision],
        )
        .map_err(invalid)?;
    let mut stmt = store
        .0
        .prepare("SELECT id,platform,puuid,summoner_id,riot_id FROM accounts")
        .map_err(invalid)?;
    let accounts = stmt
        .query_map([], |r| {
            Ok(Account {
                id: r.get(0)?,
                platform: r.get(1)?,
                puuid: r.get(2)?,
                summoner_id: r.get(3)?,
                riot_id: r.get(4)?,
            })
        })
        .map_err(invalid)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(invalid)?;
    for account in &accounts {
        if account.id != format!("{}:{}", account.platform, account.puuid)
            || account.puuid.is_empty()
        {
            return Err("backup.invalid".into());
        }
    }
    let invalid_links: bool = store.0.query_row("SELECT EXISTS(SELECT 1 FROM participations p JOIN segments s ON p.segment_id=s.id JOIN accounts a ON p.account_id=a.id JOIN games g ON p.game_id=g.id WHERE p.account_id!=s.account_id OR a.platform!=g.platform OR (p.automatic=0 AND p.manual=0))", [], |r| r.get(0)).map_err(invalid)?;
    if invalid_links {
        return Err("backup.invalid".into());
    }
    let mut stmt = store
        .0
        .prepare("SELECT id,platform,game_number,detail,timeline FROM games")
        .map_err(invalid)?;
    let games = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        })
        .map_err(invalid)?;
    for row in games {
        let (id, platform, number, detail, timeline) = row.map_err(invalid)?;
        crate::domain::validate_game_id(&number).map_err(invalid)?;
        if id != format!("{platform}_{number}") {
            return Err("backup.invalid".into());
        }
        if let Some(raw) = detail {
            let detail: Value = serde_json::from_str(&raw).map_err(invalid)?;
            if detail["gameId"].as_u64().map(|n| n.to_string()).as_deref() != Some(&number)
                || detail["platformId"].as_str() != Some(&platform)
            {
                return Err("backup.invalid".into());
            }
        }
        if let Some(raw) = timeline {
            let timeline: Value = serde_json::from_str(&raw).map_err(invalid)?;
            if !timeline["frames"].is_array() {
                return Err("backup.invalid".into());
            }
        }
    }
    for s in store.subscriptions()? {
        if crate::regions::platform(&s.server)? != s.account.platform
            || s.label.chars().count() > 80
        {
            return Err("backup.invalid".into());
        }
    }
    let mut linked=store.0.prepare("SELECT game_id,account_id FROM participations UNION SELECT game_id,account_id FROM review_games UNION SELECT f.game_id,s.account_id FROM followed_games f JOIN subscriptions s ON s.id=f.subscription_id").map_err(invalid)?;
    let linked = linked
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(invalid)?;
    for row in linked {
        let (game, account) = row.map_err(invalid)?;
        let detail: Option<String> = store
            .0
            .query_row("SELECT detail FROM games WHERE id=?1", [&game], |r| {
                r.get(0)
            })
            .map_err(invalid)?;
        if let Some(detail) = detail {
            let account = accounts
                .iter()
                .find(|a| a.id == account)
                .ok_or("backup.invalid")?;
            validate_detail(
                &game,
                account,
                &serde_json::from_str(&detail).map_err(invalid)?,
            )
            .map_err(invalid)?;
        }
    }
    // SQLite enforces all foreign keys while inserting, including followed-game links.
    Ok(())
}
