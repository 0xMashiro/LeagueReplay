use super::*;

pub(super) fn fill_segments(db: &Connection, sessions: &mut [PlaySession]) -> Result<()> {
    let positions: std::collections::HashMap<_, _> = sessions
        .iter()
        .enumerate()
        .map(|(index, session)| (session.id.clone(), index))
        .collect();
    let ids = serde_json::to_string(&sessions.iter().map(|s| &s.id).collect::<Vec<_>>())
        .map_err(|_| "library.storageError")?;
    // One ordered stream replaces one query per session and per segment.
    // The minimum game time and segment rowid also define split boundaries.
    let mut statement = db.prepare(
        "SELECT s.session_id,s.id,s.account_id,p.id
         FROM segments s JOIN participations p ON p.segment_id=s.id
         JOIN games g ON g.id=p.game_id
         WHERE s.session_id IN (SELECT value FROM json_each(?1))
         ORDER BY s.session_id,
           MIN(COALESCE(json_extract(g.detail,'$.gameCreation'),p.first_seen)) OVER (PARTITION BY s.id),
           s.rowid,COALESCE(json_extract(g.detail,'$.gameCreation'),p.first_seen),p.rowid",
    ).map_err(sql)?;
    let mut rows = statement.query([ids]).map_err(sql)?;
    while let Some(row) = rows.next().map_err(sql)? {
        let session_id: String = row.get(0).map_err(sql)?;
        let Some(&index) = positions.get(&session_id) else {
            continue;
        };
        let segments = &mut sessions[index].segments;
        let id: String = row.get(1).map_err(sql)?;
        if segments.last().is_none_or(|segment| segment.id != id) {
            segments.push(SessionSegment {
                id,
                account_id: row.get(2).map_err(sql)?,
                match_ids: Vec::new(),
            });
        }
        segments
            .last_mut()
            .unwrap()
            .match_ids
            .push(row.get(3).map_err(sql)?);
    }
    Ok(())
}

// Share ordering with the workspace and split operation. Insertion order changes
// when an old match is backfilled and must not decide where new play belongs.
pub(super) fn ordered_segments(db: &Connection, session: &str) -> Result<Vec<SessionSegment>> {
    let mut stmt = db.prepare("SELECT s.id,s.account_id FROM segments s JOIN participations p ON p.segment_id=s.id JOIN games g ON g.id=p.game_id WHERE s.session_id=?1 GROUP BY s.id ORDER BY MIN(COALESCE(json_extract(g.detail,'$.gameCreation'),p.first_seen)),s.rowid").map_err(sql)?;
    let result = stmt
        .query_map([session], |r| {
            Ok(SessionSegment {
                id: r.get(0)?,
                account_id: r.get(1)?,
                match_ids: Vec::new(),
            })
        })
        .map_err(sql)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(sql);
    result
}

fn title(value: &str) -> Result<&str> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 80 || value.chars().any(char::is_control) {
        return Err("session.invalidTitle".into());
    }
    Ok(value)
}

fn session(db: &Connection, id: &str) -> Result<(i64, i64, Option<i64>)> {
    db.query_row(
        "SELECT started_at,last_activity,ended_at FROM sessions WHERE id=?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .optional()
    .map_err(sql)?
    .ok_or("session.changed".into())
}

fn bounds(db: &Connection, session: &str) -> Result<(i64, i64)> {
    db.query_row("SELECT MIN(COALESCE(json_extract(g.detail,'$.gameCreation'),p.first_seen)),MAX(COALESCE(json_extract(g.detail,'$.gameCreation')+json_extract(g.detail,'$.gameDuration')*1000,p.last_seen)) FROM participations p JOIN games g ON g.id=p.game_id JOIN segments s ON s.id=p.segment_id WHERE s.session_id=?1", [session], |r| Ok((r.get(0)?, r.get(1)?))).map_err(sql)
}

impl Store {
    pub fn move_matches(
        &mut self,
        observations: &[String],
        destination: Option<&str>,
        name: &str,
    ) -> Result<String> {
        if observations.is_empty()
            || observations.len() > 500
            || observations
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != observations.len()
        {
            return Err("session.changed".into());
        }
        let tx = self.0.transaction().map_err(sql)?;
        let mut records = Vec::new();
        for observation in observations {
            let row: (String, String, String, String) = tx.query_row("SELECT p.segment_id,s.session_id,p.account_id,p.state FROM participations p JOIN segments s ON s.id=p.segment_id WHERE p.id=?1", [observation], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(sql)?.ok_or("session.changed")?;
            if row.3 != "ready" {
                return Err("session.matchActive".into());
            }
            records.push((observation, row));
        }
        let target = if let Some(destination) = destination {
            session(&tx, destination)?;
            destination.to_string()
        } else {
            let target = id();
            tx.execute("INSERT INTO sessions(id,title,started_at,last_activity,ended_at,end_reason) VALUES(?1,?2,0,0,0,'organized')", params![target,title(name)?]).map_err(sql)?;
            target
        };
        let mut moved_segments = std::collections::HashMap::new();
        let mut affected = std::collections::HashSet::from([target.clone()]);
        for (observation, (segment, source, account, _)) in records {
            if source == target {
                continue;
            }
            affected.insert(source);
            let next = match moved_segments.get(&segment) {
                Some(value) => value,
                None => {
                    let next = id();
                    tx.execute(
                        "INSERT INTO segments(id,session_id,account_id) VALUES(?1,?2,?3)",
                        params![next, target, account],
                    )
                    .map_err(sql)?;
                    moved_segments.entry(segment).or_insert(next)
                }
            };
            tx.execute(
                "UPDATE participations SET segment_id=?2 WHERE id=?1",
                params![observation, next],
            )
            .map_err(sql)?;
        }
        tx.execute("DELETE FROM segments WHERE NOT EXISTS(SELECT 1 FROM participations WHERE segment_id=segments.id)", []).map_err(sql)?;
        for session in affected {
            let count: i64 = tx
                .query_row(
                    "SELECT COUNT(*) FROM segments WHERE session_id=?1",
                    [&session],
                    |r| r.get(0),
                )
                .map_err(sql)?;
            if count == 0 {
                tx.execute(
                    "DELETE FROM sessions WHERE id=?1 AND ended_at IS NOT NULL",
                    [&session],
                )
                .map_err(sql)?;
            } else {
                let (start, end) = bounds(&tx, &session)?;
                tx.execute("UPDATE sessions SET started_at=?2,last_activity=CASE WHEN ended_at IS NULL THEN last_activity ELSE ?3 END,ended_at=CASE WHEN ended_at IS NULL THEN NULL ELSE ?3 END WHERE id=?1", params![session,start,end]).map_err(sql)?;
            }
        }
        tx.commit().map_err(sql)?;
        Ok(target)
    }

    pub fn rename_session(&self, session: &str, name: &str) -> Result<()> {
        if self
            .0
            .execute(
                "UPDATE sessions SET title=?2 WHERE id=?1",
                params![session, title(name)?],
            )
            .map_err(sql)?
            != 1
        {
            return Err("session.changed".into());
        }
        Ok(())
    }

    /// Keep the destination's identity/title and every segment/participation ID.
    /// An open recording stays open and its activity clock is not advanced.
    pub fn merge_sessions(&mut self, source: &str, destination: &str) -> Result<()> {
        if source == destination {
            return Err("session.sameDestination".into());
        }
        let tx = self.0.transaction().map_err(sql)?;
        let a = session(&tx, source)?;
        let b = session(&tx, destination)?;
        let end = a.2.zip(b.2).map(|(x, y)| x.max(y));
        let activity = match (a.2, b.2) {
            (None, _) => a.1,
            (_, None) => b.1,
            _ => a.1.max(b.1),
        };
        tx.execute(
            "UPDATE segments SET session_id=?2 WHERE session_id=?1",
            params![source, destination],
        )
        .map_err(sql)?;
        tx.execute("DELETE FROM sessions WHERE id=?1", [source])
            .map_err(sql)?;
        tx.execute("UPDATE sessions SET started_at=?2,last_activity=?3,ended_at=?4,end_reason=CASE WHEN ?4 IS NULL THEN NULL ELSE 'merge' END WHERE id=?1", params![destination, a.0.min(b.0), activity, end]).map_err(sql)?;
        tx.commit().map_err(sql)
    }

    pub fn split_session(&mut self, original: &str, from_segment: &str, name: &str) -> Result<()> {
        let name = title(name)?;
        let tx = self.0.transaction().map_err(sql)?;
        let (_, activity, end) = session(&tx, original)?;
        let segments = ordered_segments(&tx, original)?;
        let index = segments
            .iter()
            .position(|s| s.id == from_segment)
            .filter(|&index| index > 0)
            .ok_or("session.changed")?;
        if end.is_none() {
            let latest: Option<String> = tx.query_row("SELECT p.segment_id FROM participations p JOIN segments s ON s.id=p.segment_id WHERE s.session_id=?1 AND p.automatic=1 ORDER BY p.last_seen DESC,p.rowid DESC LIMIT 1", [original], |r| r.get(0)).optional().map_err(sql)?;
            if latest.is_some_and(|last| segments[..index].iter().any(|s| s.id == last)) {
                return Err("session.activePrefix".into());
            }
        }
        // Release the single open-session slot before transferring it to the tail.
        tx.execute("UPDATE sessions SET ended_at=COALESCE(ended_at,last_activity),end_reason='split' WHERE id=?1", [original]).map_err(sql)?;
        let new_id = id();
        tx.execute("INSERT INTO sessions(id,title,started_at,last_activity,ended_at,end_reason) VALUES (?1,?2,?3,?3,?4,CASE WHEN ?4 IS NULL THEN NULL ELSE 'split' END)", params![new_id, name, activity, end]).map_err(sql)?;
        for segment in &segments[index..] {
            tx.execute(
                "UPDATE segments SET session_id=?2 WHERE id=?1",
                params![segment.id, new_id],
            )
            .map_err(sql)?;
        }
        let prefix = bounds(&tx, original)?;
        let tail = bounds(&tx, &new_id)?;
        tx.execute(
            "UPDATE sessions SET started_at=?2,last_activity=?3,ended_at=?3 WHERE id=?1",
            params![original, prefix.0, prefix.1],
        )
        .map_err(sql)?;
        tx.execute("UPDATE sessions SET started_at=?2,ended_at=CASE WHEN ended_at IS NULL THEN NULL ELSE MAX(ended_at,?3) END WHERE id=?1", params![new_id, tail.0, tail.1]).map_err(sql)?;
        tx.commit().map_err(sql)
    }
}
