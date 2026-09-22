pub mod backup;
#[cfg(test)]
mod benchmarks;
mod following;
mod identity;
mod observed;
mod play;
mod play_filter;
mod replays;
mod review;
mod schema;
#[cfg(test)]
mod session_tests;
mod sessions;
mod sync;
#[cfg(test)]
mod tests;

pub type SharedArchive = std::sync::Arc<std::sync::Mutex<Store>>;

use crate::domain::*;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use uuid::Uuid;

pub const IDLE_MS: i64 = 60 * 60 * 1000;
pub struct Store(Connection, std::cell::RefCell<Option<(i64, PlaySummary)>>);
type Result<T> = std::result::Result<T, String>;
fn sql(_: rusqlite::Error) -> String {
    "library.storageError".into()
}

fn save_timeline(
    connection: &rusqlite::Transaction<'_>,
    game: &str,
    timeline: Option<&Value>,
    error: Option<&str>,
    now: i64,
) -> Result<()> {
    // Both callers run this inside their detail transaction. A late failure cannot
    // replace an existing timeline or leave stale errors after a concurrent success.
    connection
        .execute(
            "UPDATE games SET timeline=COALESCE(?2,timeline),
         data_error=CASE WHEN COALESCE(?2,timeline) IS NULL THEN ?3 ELSE NULL END,
         retry_at=CASE WHEN COALESCE(?2,timeline) IS NULL THEN ?4 ELSE 0 END WHERE id=?1",
            params![game, timeline.map(Value::to_string), error, now + 60_000],
        )
        .map_err(sql)?;
    connection.execute(
        "UPDATE review_games SET timeline_error=(SELECT data_error FROM games WHERE id=?1) WHERE game_id=?1",
        [game],
    ).map_err(sql)?;
    Ok(())
}
fn id() -> String {
    Uuid::new_v4().to_string()
}

pub(crate) fn store_participant_accounts(
    connection: &Connection,
    platform: &str,
    detail: &Value,
) -> Result<()> {
    for account in crate::matches::participant_accounts(platform, detail) {
        connection.execute("INSERT INTO accounts(id,platform,puuid,summoner_id,riot_id) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET riot_id=excluded.riot_id,summoner_id=excluded.summoner_id",params![account.id,account.platform,account.puuid,account.summoner_id,account.riot_id]).map_err(sql)?;
    }
    Ok(())
}

impl Store {
    #[cfg(test)]
    pub(crate) fn test_connection(&mut self) -> &mut Connection {
        &mut self.0
    }

    pub fn new(mut connection: Connection) -> Result<Self> {
        schema::open(&mut connection)?;
        Ok(Self(connection, std::cell::RefCell::new(None)))
    }

    pub fn observe(&mut self, sample: Option<&Sample>, now: i64) -> Result<()> {
        // One transaction freezes ownership, session assignment and activity together.
        let transaction = self.0.transaction().map_err(sql)?;
        if let Some(sample) = sample.filter(|s| !s.is_playing()) {
            if let Some(game) = &sample.observed_game {
                // A newly confirmed settlement supersedes a pre-settlement failure.
                // Only reset once; repeated end-screen samples preserve backoff.
                transaction.execute("UPDATE games SET retry_at=0,data_error=NULL WHERE id=?1 AND detail IS NULL AND EXISTS(SELECT 1 FROM participations WHERE game_id=?1 AND account_id=?2 AND state='playing')",params![game.id,sample.account.id]).map_err(sql)?;
            }
        }
        transaction
            .execute(
                "UPDATE participations SET state='pending' WHERE state='playing'",
                [],
            )
            .map_err(sql)?;
        let open: Option<(String, i64)> = transaction
            .query_row(
                "SELECT id,last_activity FROM sessions WHERE ended_at IS NULL",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(sql)?;
        let existing: Option<(String, String)> = if let Some(sample) = sample {
            if let Some(game) = &sample.observed_game {
                transaction.query_row("SELECT p.id,s.session_id FROM participations p JOIN segments s ON s.id=p.segment_id WHERE p.game_id=?1 AND p.account_id=?2", params![game.id,sample.account.id], |r| Ok((r.get(0)?,r.get(1)?))).optional().map_err(sql)?
            } else {
                None
            }
        } else {
            None
        };
        let continuing = sample.is_some_and(Sample::is_playing)
            && open.as_ref().is_some_and(|(session, _)| {
                existing.as_ref().is_some_and(|(_, old)| old == session)
            });
        if let Some((session, last)) = &open {
            if now.saturating_sub(*last) >= IDLE_MS && !continuing {
                transaction
                    .execute(
                        "UPDATE sessions SET ended_at=?2,end_reason='idle' WHERE id=?1",
                        params![session, last + IDLE_MS],
                    )
                    .map_err(sql)?;
            }
        }
        if let Some(sample) = sample {
            let a = &sample.account;
            transaction.execute("INSERT INTO accounts(id,platform,puuid,summoner_id,riot_id) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET riot_id=excluded.riot_id,summoner_id=excluded.summoner_id",params![a.id,a.platform,a.puuid,a.summoner_id,a.riot_id]).map_err(sql)?;
            if let Some(game) = &sample.observed_game {
                let state = if sample.is_playing() {
                    "playing"
                } else {
                    "pending"
                };
                transaction
                    .execute(
                        "INSERT OR IGNORE INTO games(id,platform,game_number) VALUES (?1,?2,?3)",
                        params![game.id, a.platform, game.game_id],
                    )
                    .map_err(sql)?;
                if let Some((observation, _)) = existing {
                    transaction.execute("UPDATE participations SET automatic=1,last_seen=?2,state=CASE WHEN (SELECT detail FROM games WHERE id=game_id) IS NULL THEN ?3 ELSE 'ready' END WHERE id=?1",params![observation,now,state]).map_err(sql)?;
                } else {
                    let session: Option<String> = transaction
                        .query_row("SELECT id FROM sessions WHERE ended_at IS NULL", [], |r| {
                            r.get(0)
                        })
                        .optional()
                        .map_err(sql)?;
                    let session = match session {
                        Some(id) => id,
                        None => {
                            let session = id();
                            transaction.execute("INSERT INTO sessions(id,title,started_at,last_activity) VALUES (?1,'游玩场次',?2,?2)",params![session,now]).map_err(sql)?;
                            session
                        }
                    };
                    let last_segment: Option<(String,String)> = transaction.query_row("SELECT s.id,s.account_id FROM segments s JOIN participations p ON p.segment_id=s.id JOIN games g ON g.id=p.game_id WHERE s.session_id=?1 GROUP BY s.id ORDER BY MAX(COALESCE(json_extract(g.detail,'$.gameCreation'),p.first_seen)) DESC,s.rowid DESC LIMIT 1",[&session],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(sql)?;
                    let segment = match last_segment {
                        Some((id, account)) if account == a.id => id,
                        _ => {
                            let segment = id();
                            transaction.execute("INSERT INTO segments(id,session_id,account_id) VALUES (?1,?2,?3)",params![segment,session,a.id]).map_err(sql)?;
                            segment
                        }
                    };
                    transaction.execute("INSERT INTO participations(id,game_id,account_id,segment_id,first_seen,last_seen,champion_id,queue_name,state,automatic,manual) VALUES (?1,?2,?3,?4,?5,?5,?6,?7,?8,1,0)",params![id(),game.id,a.id,segment,now,game.champion_id,game.queue_name,state]).map_err(sql)?;
                }
            }
            if sample.queue_active {
                transaction
                    .execute(
                        "UPDATE sessions SET last_activity=?1 WHERE ended_at IS NULL",
                        [now],
                    )
                    .map_err(sql)?;
            }
        }
        transaction.commit().map_err(sql)
    }

    pub fn end_session(&mut self, session: &str, now: i64) -> Result<()> {
        let changed=self.0.execute("UPDATE sessions SET ended_at=?2,end_reason='manual' WHERE id=?1 AND ended_at IS NULL",params![session,now]).map_err(sql)?;
        if changed != 1 {
            return Err("session.ended".into());
        }
        Ok(())
    }

    pub fn pending(&self, platform: &str, now: i64) -> Result<Option<(String, String, Account)>> {
        self.0.query_row("WITH candidates AS (
            SELECT game_id,account_id,0 AS priority FROM participations WHERE state='pending'
            UNION SELECT game_id,account_id,1 FROM participations WHERE state='ready'
            UNION SELECT game_id,account_id,1 FROM review_games
        ) SELECT g.id,g.game_number,a.id,a.platform,a.puuid,a.summoner_id,a.riot_id
        FROM candidates c JOIN games g ON c.game_id=g.id JOIN accounts a ON a.id=c.account_id
        WHERE g.platform=?1 AND g.retry_at<=?2 AND (c.priority=0 OR (g.detail IS NOT NULL AND g.timeline IS NULL))
        ORDER BY c.priority,g.retry_at,g.id LIMIT 1",params![platform,now],|r| Ok((r.get(0)?,r.get(1)?,Account{id:r.get(2)?,platform:r.get(3)?,puuid:r.get(4)?,summoner_id:r.get(5)?,riot_id:r.get(6)?}))).optional().map_err(sql)
    }

    pub fn failed(&mut self, game: &str, now: i64, message: &str) -> Result<()> {
        let tx = self.0.transaction().map_err(sql)?;
        tx
            .execute(
                "UPDATE games SET data_error=?2,retry_at=?3 WHERE id=?1 AND (detail IS NULL OR timeline IS NULL)",
                params![game, message, now + 60_000],
            )
            .map_err(sql)?;
        tx.execute("UPDATE review_games SET timeline_error=(SELECT data_error FROM games WHERE id=?1) WHERE game_id=?1 AND EXISTS(SELECT 1 FROM games WHERE id=?1 AND timeline IS NULL)", [game]).map_err(sql)?;
        tx.commit().map_err(sql)
    }

    pub fn complete(
        &mut self,
        game: &str,
        account: &Account,
        detail: &Value,
        timeline: std::result::Result<&Value, &str>,
        now: i64,
    ) -> Result<()> {
        validate_detail(game, account, detail)?;
        let transaction = self.0.transaction().map_err(sql)?;
        store_participant_accounts(&transaction, &account.platform, detail)?;
        transaction
            .execute(
                "UPDATE games SET detail=?2 WHERE id=?1",
                params![game, detail.to_string()],
            )
            .map_err(sql)?;
        transaction
            .execute(
                "UPDATE participations SET state='ready' WHERE game_id=?1",
                [game],
            )
            .map_err(sql)?;
        save_timeline(&transaction, game, timeline.ok(), timeline.err(), now)?;
        transaction.commit().map_err(sql)
    }

    /// Manual attribution adds provenance to the same (game, account) relation.
    /// It never refreshes the activity clock or changes automatic ownership.
    pub fn import(
        &mut self,
        account: &Account,
        detail: &Value,
        timeline: Option<&Value>,
        session: Option<&str>,
        now: i64,
    ) -> Result<()> {
        let transaction = self.0.transaction().map_err(sql)?;
        Self::import_into(&transaction, account, detail, timeline, session, now)?;
        transaction.commit().map_err(sql)
    }

    fn import_into(
        transaction: &Connection,
        account: &Account,
        detail: &Value,
        timeline: Option<&Value>,
        session: Option<&str>,
        now: i64,
    ) -> Result<()> {
        let game = format!(
            "{}_{}",
            account.platform,
            detail["gameId"].as_u64().ok_or("game.invalidId")?
        );
        let participant = validate_detail(&game, account, detail)?;
        transaction.execute("INSERT INTO accounts(id,platform,puuid,summoner_id,riot_id) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET riot_id=excluded.riot_id,summoner_id=excluded.summoner_id",params![account.id,account.platform,account.puuid,account.summoner_id,account.riot_id]).map_err(sql)?;
        store_participant_accounts(transaction, &account.platform, detail)?;
        transaction.execute("INSERT INTO games(id,platform,game_number,detail,timeline) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET detail=excluded.detail,timeline=COALESCE(excluded.timeline,games.timeline),data_error=NULL",params![game,account.platform,detail["gameId"].as_u64().unwrap().to_string(),detail.to_string(),timeline.map(Value::to_string)]).map_err(sql)?;
        let existing: Option<String> = transaction
            .query_row(
                "SELECT id FROM participations WHERE game_id=?1 AND account_id=?2",
                params![game, account.id],
                |r| r.get(0),
            )
            .optional()
            .map_err(sql)?;
        if let Some(existing) = existing {
            if let Some(destination) = session {
                let current: String = transaction.query_row("SELECT s.session_id FROM segments s JOIN participations p ON p.segment_id=s.id WHERE p.id=?1", [&existing], |r| r.get(0)).map_err(sql)?;
                if current != destination {
                    return Err("session.alreadyAttributed".into());
                }
            }
            transaction
                .execute(
                    "UPDATE participations SET manual=1,state='ready' WHERE id=?1",
                    [existing],
                )
                .map_err(sql)?;
        } else {
            let start = detail["gameCreation"]
                .as_i64()
                .filter(|v| *v > 0)
                .ok_or("game.missingStartTime")?;
            let end = start.saturating_add(
                detail["gameDuration"]
                    .as_i64()
                    .unwrap_or(0)
                    .saturating_mul(1000),
            );
            let session = match session {
                Some(session) => {
                    let exists: bool = transaction
                        .query_row(
                            "SELECT EXISTS(SELECT 1 FROM sessions WHERE id=?1)",
                            [session],
                            |r| r.get(0),
                        )
                        .map_err(sql)?;
                    if !exists {
                        return Err("session.changed".into());
                    }
                    session.to_owned()
                }
                None => {
                    let session = id();
                    transaction.execute("INSERT INTO sessions(id,title,started_at,last_activity,ended_at,end_reason) VALUES (?1,'补录场次',?2,?2,?3,'import')",params![session,start,end]).map_err(sql)?;
                    session
                }
            };
            let segment = id();
            transaction
                .execute(
                    "INSERT INTO segments(id,session_id,account_id) VALUES (?1,?2,?3)",
                    params![segment, session, account.id],
                )
                .map_err(sql)?;
            transaction.execute("INSERT INTO participations(id,game_id,account_id,segment_id,first_seen,last_seen,champion_id,queue_name,state,automatic,manual) VALUES (?1,?2,?3,?4,?5,?5,?6,?7,'ready',0,1)",params![id(),game,account.id,segment,now,participant["championId"].as_i64().unwrap_or(0),format!("队列 {}",detail["queueId"])]).map_err(sql)?;
            transaction
                .execute(
                    "UPDATE sessions SET started_at=MIN(started_at,?2),ended_at=CASE WHEN ended_at IS NULL THEN NULL ELSE MAX(ended_at,?3) END WHERE id=?1",
                    params![session, start, end],
                )
                .map_err(sql)?;
        }
        Ok(())
    }

    pub fn import_many(
        &mut self,
        games: &[crate::domain::review::StoredReview],
        destination: Option<&str>,
        name: &str,
    ) -> Result<()> {
        if games.is_empty() || games.len() > 50 {
            return Err("session.changed".into());
        }
        let tx = self.0.transaction().map_err(sql)?;
        let target = match destination {
            Some(value) => value.to_string(),
            None => {
                let value = id();
                if name.trim().is_empty()
                    || name.chars().count() > 80
                    || name.chars().any(char::is_control)
                {
                    return Err("session.invalidTitle".into());
                }
                let start = games
                    .iter()
                    .filter_map(|g| g.detail["gameCreation"].as_i64())
                    .min()
                    .ok_or("game.invalidData")?;
                tx.execute("INSERT INTO sessions(id,title,started_at,last_activity,ended_at,end_reason) VALUES(?1,?2,?3,?3,?3,'import')", params![value,name.trim(),start]).map_err(sql)?;
                value
            }
        };
        for game in games {
            validate_detail(&game.id, &game.account, &game.detail)?;
            Self::import_into(
                &tx,
                &game.account,
                &game.detail,
                game.timeline.as_ref(),
                Some(&target),
                now(),
            )?;
        }
        tx.commit().map_err(sql)
    }

    pub fn retract_manual(&mut self, observation: &str) -> Result<()> {
        let transaction = self.0.transaction().map_err(sql)?;
        transaction
            .execute(
                "UPDATE participations SET manual=0 WHERE id=?1",
                [observation],
            )
            .map_err(sql)?;
        transaction
            .execute(
                "DELETE FROM participations WHERE id=?1 AND automatic=0",
                [observation],
            )
            .map_err(sql)?;
        transaction.execute("DELETE FROM segments WHERE NOT EXISTS(SELECT 1 FROM participations WHERE segment_id=segments.id)",[]).map_err(sql)?;
        transaction.execute("DELETE FROM sessions WHERE NOT EXISTS(SELECT 1 FROM segments WHERE session_id=sessions.id)",[]).map_err(sql)?;
        // Keep the canonical game and notes: retracting attribution is not deletion.
        transaction.commit().map_err(sql)
    }

    pub fn workspace(&self, status: ClientStatus) -> Result<LiveWorkspace> {
        self.workspace_since(status, None)
    }
    fn workspace_since(&self, status: ClientStatus, cursor: Option<i64>) -> Result<LiveWorkspace> {
        let mut stmt = self
            .0
            .prepare("SELECT id,platform,puuid,summoner_id,riot_id FROM accounts ORDER BY riot_id")
            .map_err(sql)?;
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
            .map_err(sql)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sql)?;
        let mut stmt=self.0.prepare("SELECT g.id,g.platform,g.game_number,p.account_id,p.segment_id,p.first_seen,p.last_seen,p.champion_id,p.queue_name,p.state,g.detail,g.data_error,p.id,p.automatic,p.manual FROM participations p JOIN games g ON p.game_id=g.id WHERE ?1 IS NULL OR p.id IN (SELECT id FROM workspace_changes WHERE kind='participation' AND revision>?1) OR g.id IN (SELECT id FROM workspace_changes WHERE kind='game' AND revision>?1) ORDER BY COALESCE(json_extract(g.detail,'$.gameCreation'),p.first_seen)").map_err(sql)?;
        let matches = stmt
            .query_map([cursor], observed::read)
            .map_err(sql)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sql)?;
        let mut stmt=self.0.prepare("SELECT id,title,started_at,ended_at,end_reason FROM sessions ORDER BY started_at DESC").map_err(sql)?;
        let mut sessions = stmt
            .query_map([], |r| {
                Ok(PlaySession {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    started_at: r.get::<_, i64>(2)? as f64,
                    ended_at: r.get::<_, Option<i64>>(3)?.map(|v| v as f64),
                    end_reason: r.get(4)?,
                    segments: Vec::new(),
                })
            })
            .map_err(sql)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sql)?;
        sessions::fill_segments(&self.0, &mut sessions)?;
        let (raw, revision): (String, u32) = self
            .0
            .query_row("SELECT data,revision FROM user_state WHERE id=1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .map_err(sql)?;
        let ui = serde_json::from_str(&raw).map_err(|_| "ui.unreadable")?;
        Ok(LiveWorkspace {
            status,
            accounts,
            matches,
            sessions,
            ui,
            ui_revision: revision,
            idle_minutes: 60,
        })
    }

    pub fn save_ui(&mut self, ui: &LiveUiState, revision: u32) -> Result<u32> {
        identity::validate(ui)?;
        let raw = serde_json::to_string(ui).map_err(|e| e.to_string())?;
        if raw.len() > 2 * 1024 * 1024 {
            return Err("ui.tooLarge".into());
        }
        if ui
            .theme
            .as_ref()
            .is_some_and(|v| !["dark", "light", "system"].contains(&v.as_str()))
        {
            return Err("ui.invalidTheme".into());
        }
        if ui
            .notes
            .iter()
            .any(|n| n.body.trim().is_empty() || n.at.is_some_and(|v| !v.is_finite() || v < 0.0))
        {
            return Err("ui.invalidNote".into());
        }
        if ui.players.iter().any(|p| {
            p.name.trim().is_empty()
                || p.name.chars().count() > 80
                || p.name.chars().any(char::is_control)
        }) {
            return Err("ui.invalidPlayerName".into());
        }
        let mut ids = std::collections::HashSet::new();
        for player in &ui.players {
            for account in &player.account_ids {
                if !ids.insert(account) {
                    return Err("identity.alreadyLinked".into());
                }
            }
        }
        let changed = self
            .0
            .execute(
                "UPDATE user_state SET data=?1,revision=revision+1 WHERE id=1 AND revision=?2",
                params![raw, revision],
            )
            .map_err(sql)?;
        if changed != 1 {
            return Err("ui.conflict".into());
        }
        Ok(revision + 1)
    }
}

pub fn validate_detail<'a>(game: &str, account: &Account, detail: &'a Value) -> Result<&'a Value> {
    crate::domain::validate_game_id(
        &detail["gameId"]
            .as_u64()
            .ok_or("game.invalidId")?
            .to_string(),
    )?;
    let platform = detail["platformId"].as_str().ok_or("game.invalidData")?;
    if !platform.eq_ignore_ascii_case(&account.platform)
        || detail["gameId"]
            .as_u64()
            .map(|n| format!("{}_{n}", account.platform))
            .as_deref()
            != Some(game)
    {
        return Err("game.identityMismatch".into());
    }
    if detail["gameDuration"].as_u64().unwrap_or(0) == 0 {
        return Err("game.notFinished".into());
    }
    let identity = detail["participantIdentities"]
        .as_array()
        .and_then(|list| {
            list.iter()
                .find(|p| p["player"]["puuid"].as_str() == Some(account.puuid.as_str()))
        })
        .ok_or("game.playerMissing")?;
    let participant = detail["participants"]
        .as_array()
        .and_then(|list| {
            list.iter()
                .find(|p| p["participantId"] == identity["participantId"])
        })
        .ok_or("game.playerMissing")?;
    if participant["stats"]["win"].as_bool().is_none() {
        return Err("game.notFinished".into());
    }
    Ok(participant)
}
